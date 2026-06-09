import { useEffect, useState } from "react";
import { getMcpServers, getMcpBundles } from "../lib/api";
import type { McpServerEntry, McpBundleEntry } from "../types/api";

export default function McpDashboard() {
  const [servers, setServers] = useState<McpServerEntry[]>([]);
  const [bundles, setBundles] = useState<McpBundleEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([
      getMcpServers().then((r) => r.servers).catch(() => [] as McpServerEntry[]),
      getMcpBundles().then((r) => r.bundles).catch(() => [] as McpBundleEntry[]),
    ])
      .then(([srv, bnd]) => {
        setServers(srv);
        setBundles(bnd);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return (
      <div className="min-h-[60vh] flex items-center justify-center">
        <div
          className="h-8 w-8 border-2 rounded-full animate-spin"
          style={{ borderColor: "var(--pc-border)", borderTopColor: "var(--pc-accent)" }}
        />
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6">
        <h1 className="text-xl font-semibold mb-4">MCP Dashboard</h1>
        <div className="rounded-lg border border-red-200 bg-red-50 p-4 text-red-700">
          Failed to load MCP data: {error}
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-xl font-semibold">MCP Dashboard</h1>

      <section>
        <h2 className="text-lg font-medium mb-3">Servers</h2>
        {servers.length === 0 ? (
          <div className="rounded-lg border p-4 text-muted-foreground">
            No MCP servers configured.
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {servers.map((srv) => (
              <div
                key={srv.name}
                className="rounded-lg border p-4"
                style={{ borderColor: "var(--pc-border)" }}
              >
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium">{srv.name}</h3>
                  <span
                    className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${
                      srv.enabled
                        ? "bg-green-100 text-green-800"
                        : "bg-gray-100 text-gray-800"
                    }`}
                  >
                    {srv.enabled ? "Enabled" : "Disabled"}
                  </span>
                </div>
                {srv.command && (
                  <p className="text-sm text-muted-foreground">
                    Command: <code>{srv.command}</code>
                  </p>
                )}
                {srv.url && (
                  <p className="text-sm text-muted-foreground">
                    URL: <code>{srv.url}</code>
                  </p>
                )}
                {srv.tools && srv.tools.length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-1">
                    {srv.tools.map((t) => (
                      <span
                        key={t}
                        className="inline-flex items-center rounded-md bg-purple-50 px-2 py-1 text-xs font-medium text-purple-700"
                      >
                        {t}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </section>

      <section>
        <h2 className="text-lg font-medium mb-3">Bundles</h2>
        {bundles.length === 0 ? (
          <div className="rounded-lg border p-4 text-muted-foreground">
            No MCP bundles configured.
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {bundles.map((bnd) => (
              <div
                key={bnd.name}
                className="rounded-lg border p-4"
                style={{ borderColor: "var(--pc-border)" }}
              >
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium">{bnd.name}</h3>
                  <span
                    className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${
                      bnd.enabled
                        ? "bg-green-100 text-green-800"
                        : "bg-gray-100 text-gray-800"
                    }`}
                  >
                    {bnd.enabled ? "Enabled" : "Disabled"}
                  </span>
                </div>
                <p className="text-sm text-muted-foreground">
                  Servers: {bnd.servers.join(", ") || "None"}
                </p>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
