import { useEffect, useState } from "react";
import { getPlugins } from "../lib/api";
import type { PluginListResponse, PluginInfo } from "../types/api";

export default function Plugins() {
  const [data, setData] = useState<PluginListResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getPlugins()
      .then(setData)
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
        <h1 className="text-xl font-semibold mb-4">Plugins</h1>
        <div className="rounded-lg border border-red-200 bg-red-50 p-4 text-red-700">
          Failed to load plugins: {error}
        </div>
      </div>
    );
  }

  return (
    <div className="p-6">
      <h1 className="text-xl font-semibold mb-4">Plugins</h1>

      {!data?.plugins_enabled && (
        <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 text-amber-800 mb-4">
          Plugin support is currently disabled. Enable it in configuration to load WASM plugins.
        </div>
      )}

      {data?.plugins_enabled && data.plugins.length === 0 && (
        <div className="rounded-lg border p-4 text-muted-foreground">
          No plugins found in <code>{data.plugins_dir}</code>.
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {data?.plugins.map((plugin: PluginInfo) => (
          <div
            key={plugin.name}
            className="rounded-lg border p-4 transition-shadow hover:shadow-md"
            style={{ borderColor: "var(--pc-border)" }}
          >
            <div className="flex items-center justify-between mb-2">
              <h2 className="font-medium">{plugin.name}</h2>
              <span
                className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${
                  plugin.loaded
                    ? "bg-green-100 text-green-800"
                    : "bg-gray-100 text-gray-800"
                }`}
              >
                {plugin.loaded ? "Loaded" : "Unloaded"}
              </span>
            </div>
            <p className="text-sm text-muted-foreground mb-2">{plugin.description || "No description"}</p>
            <p className="text-xs text-muted-foreground">Version: {plugin.version || "unknown"}</p>
            {plugin.capabilities.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1">
                {plugin.capabilities.map((cap) => (
                  <span
                    key={cap}
                    className="inline-flex items-center rounded-md bg-blue-50 px-2 py-1 text-xs font-medium text-blue-700"
                  >
                    {cap}
                  </span>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
