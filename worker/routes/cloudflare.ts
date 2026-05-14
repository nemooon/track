import { Hono } from "hono";
import type { Env, AuthVars } from "../types";

const cloudflare = new Hono<{ Bindings: Env; Variables: AuthVars }>();

const GRAPHQL_ENDPOINT = "https://api.cloudflare.com/client/v4/graphql";

const FREE_LIMITS = {
  aiNeurons: 10_000,
  workersRequests: 100_000,
  d1ReadQueries: 5_000_000,
  d1WriteQueries: 100_000,
  d1StorageBytes: 5 * 1024 * 1024 * 1024,
} as const;

type GraphQLResponse<T> = {
  data?: T;
  errors?: Array<{ message: string }>;
};

async function gql<T>(
  env: Env,
  label: string,
  query: string,
  variables: Record<string, unknown>,
): Promise<T | null> {
  const res = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${env.CF_API_TOKEN}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ query, variables }),
  });
  if (!res.ok) {
    console.error(`[gql ${label}] http`, res.status, await res.text());
    return null;
  }
  const body = (await res.json()) as GraphQLResponse<T>;
  if (body.errors && body.errors.length > 0) {
    console.error(`[gql ${label}] errors`, JSON.stringify(body.errors));
    return null;
  }
  return body.data ?? null;
}

function sumField(
  groups: Array<{ sum?: Record<string, number | undefined> }> | undefined,
  field: string,
): number {
  if (!groups) return 0;
  let total = 0;
  for (const g of groups) {
    total += g.sum?.[field] ?? 0;
  }
  return total;
}

cloudflare.get("/usage", async (c) => {
  const env = c.env;
  if (!env.CF_ACCOUNT_ID || !env.CF_API_TOKEN) {
    return c.json({ setup_required: true }, 200);
  }

  const cache = (caches as unknown as { default: Cache }).default;
  const cacheKey = new Request(
    new URL("/__usage_cache", c.req.url).toString(),
    { method: "GET" },
  );
  const cached = await cache.match(cacheKey);
  if (cached) {
    return new Response(cached.body, {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  }

  const now = new Date();
  const startOfDay = new Date(
    Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate(), 0, 0, 0),
  );
  const datetimeStart = startOfDay.toISOString();
  const datetimeEnd = now.toISOString();
  const dateStart = datetimeStart.slice(0, 10);
  const dateEnd = dateStart;

  const accountTag = env.CF_ACCOUNT_ID;
  const storageRangeStart = new Date(startOfDay.getTime() - 2 * 24 * 60 * 60 * 1000)
    .toISOString()
    .slice(0, 10);

  const aiQuery = `
    query($accountTag: string!, $start: Time!, $end: Time!) {
      viewer {
        accounts(filter: { accountTag: $accountTag }) {
          aiInferenceAdaptiveGroups(
            limit: 1000
            filter: { datetime_geq: $start, datetime_leq: $end }
          ) {
            count
            sum { totalNeurons }
          }
        }
      }
    }
  `;
  const workersQuery = `
    query($accountTag: string!, $start: Time!, $end: Time!) {
      viewer {
        accounts(filter: { accountTag: $accountTag }) {
          workersInvocationsAdaptive(
            limit: 10000
            filter: { datetime_geq: $start, datetime_leq: $end }
          ) {
            sum { requests errors }
          }
        }
      }
    }
  `;
  const d1Query = `
    query($accountTag: string!, $start: Date!, $end: Date!) {
      viewer {
        accounts(filter: { accountTag: $accountTag }) {
          d1AnalyticsAdaptiveGroups(
            limit: 1000
            filter: { date_geq: $start, date_leq: $end }
          ) {
            sum { readQueries writeQueries }
          }
        }
      }
    }
  `;
  const d1StorageQuery = `
    query($accountTag: string!, $start: Date!, $end: Date!) {
      viewer {
        accounts(filter: { accountTag: $accountTag }) {
          d1StorageAdaptiveGroups(
            limit: 1000
            filter: { date_geq: $start, date_leq: $end }
            orderBy: [date_DESC]
          ) {
            max { databaseSizeBytes }
            dimensions { databaseId date }
          }
        }
      }
    }
  `;

  type AccountWrap<K extends string, V> = {
    viewer: { accounts: Array<Record<K, V[]>> };
  };

  const [aiData, workersData, d1Data, d1StorageData] = await Promise.all([
    gql<AccountWrap<"aiInferenceAdaptiveGroups", { count?: number; sum?: { totalNeurons?: number } }>>(
      env,
      "ai",
      aiQuery,
      { accountTag, start: datetimeStart, end: datetimeEnd },
    ),
    gql<AccountWrap<"workersInvocationsAdaptive", { sum?: { requests?: number; errors?: number } }>>(
      env,
      "workers",
      workersQuery,
      { accountTag, start: datetimeStart, end: datetimeEnd },
    ),
    gql<AccountWrap<"d1AnalyticsAdaptiveGroups", { sum?: { readQueries?: number; writeQueries?: number } }>>(
      env,
      "d1",
      d1Query,
      { accountTag, start: dateStart, end: dateEnd },
    ),
    gql<AccountWrap<"d1StorageAdaptiveGroups", { max?: { databaseSizeBytes?: number }; dimensions?: { databaseId: string; date: string } }>>(
      env,
      "d1-storage",
      d1StorageQuery,
      { accountTag, start: storageRangeStart, end: dateEnd },
    ),
  ]);

  const aiGroups = aiData?.viewer.accounts[0]?.aiInferenceAdaptiveGroups;
  const workersGroups = workersData?.viewer.accounts[0]?.workersInvocationsAdaptive;
  const d1Groups = d1Data?.viewer.accounts[0]?.d1AnalyticsAdaptiveGroups;
  const d1StorageGroups = d1StorageData?.viewer.accounts[0]?.d1StorageAdaptiveGroups;

  const latestSizeByDb = new Map<string, { date: string; size: number }>();
  for (const g of d1StorageGroups ?? []) {
    const dims = (g as { dimensions?: { databaseId: string; date: string } })
      .dimensions;
    const size = (g as { max?: { databaseSizeBytes?: number } }).max
      ?.databaseSizeBytes;
    if (!dims || size == null) continue;
    const prev = latestSizeByDb.get(dims.databaseId);
    if (!prev || dims.date > prev.date) {
      latestSizeByDb.set(dims.databaseId, { date: dims.date, size });
    }
  }
  const totalStorageBytes = [...latestSizeByDb.values()].reduce(
    (acc, v) => acc + v.size,
    0,
  );

  const tomorrowUtc = new Date(startOfDay.getTime() + 24 * 60 * 60 * 1000);

  const payload = {
    setup_required: false,
    asOf: now.toISOString(),
    resetAt: tomorrowUtc.toISOString(),
    ai: {
      ok: aiData !== null,
      neurons: sumField(aiGroups, "totalNeurons"),
      inferences: (aiGroups ?? []).reduce(
        (acc, g) => acc + ((g as { count?: number }).count ?? 0),
        0,
      ),
      limit: FREE_LIMITS.aiNeurons,
    },
    workers: {
      ok: workersData !== null,
      requests: sumField(workersGroups, "requests"),
      errors: sumField(workersGroups, "errors"),
      limit: FREE_LIMITS.workersRequests,
    },
    d1: {
      ok: d1Data !== null,
      reads: sumField(d1Groups, "readQueries"),
      writes: sumField(d1Groups, "writeQueries"),
      storageBytes: totalStorageBytes,
      storageOk: d1StorageData !== null,
      readsLimit: FREE_LIMITS.d1ReadQueries,
      writesLimit: FREE_LIMITS.d1WriteQueries,
      storageLimit: FREE_LIMITS.d1StorageBytes,
    },
  };

  const body = JSON.stringify(payload);
  await cache.put(
    cacheKey,
    new Response(body, {
      headers: {
        "Content-Type": "application/json",
        "Cache-Control": "max-age=60",
      },
    }),
  );
  return new Response(body, {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});

export { cloudflare };
