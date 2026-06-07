"use client";

import { useState } from "react";

const EXPORT_TYPES = [
  {
    key: "behavior",
    label: "行为数据",
    description: "Agent 行为日志，包含决策、交易、技能使用等",
    formats: ["json", "csv"],
    path: "/api/v1/export/behavior",
  },
  {
    key: "network",
    label: "网络关系",
    description: "Agent 社交网络图，包含信任、贸易、消息等关系",
    formats: ["json", "dot", "gexf"],
    path: "/api/v1/export/network",
  },
  {
    key: "economic",
    label: "经济数据",
    description: "财富分布、GDP 时序、交易记录、银行、股市数据",
    formats: ["json", "csv"],
    path: "/api/v1/export/economic",
  },
  {
    key: "organization",
    label: "组织数据",
    description: "组织信息、提案、治理指标",
    formats: ["json", "csv"],
    path: "/api/v1/export/organization",
  },
  {
    key: "prices",
    label: "价格数据",
    description: "股票列表、已成交订单、资产分布、市场价格",
    formats: ["json", "csv"],
    path: "/api/v1/export/prices",
  },
  {
    key: "world_snapshot",
    label: "世界快照",
    description: "完整世界状态快照（V2 API）",
    formats: ["json", "csv"],
    path: "/api/v2/export/world",
  },
  {
    key: "bundle",
    label: "综合包",
    description: "一次性导出所有数据（V2 API）",
    formats: ["json", "csv"],
    path: "/api/v2/export/bundle",
  },
];

export default function ExportPage() {
  const [downloading, setDownloading] = useState<string | null>(null);

  const handleDownload = async (exportType: typeof EXPORT_TYPES[number], format: string) => {
    setDownloading(`${exportType.key}-${format}`);
    try {
      const separator = exportType.path.includes("?") ? "&" : "?";
      const url = `${exportType.path}${separator}format=${format}`;
      const response = await fetch(url);
      if (!response.ok) throw new Error(`下载失败: ${response.status}`);
      const blob = await response.blob();
      const blobUrl = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = blobUrl;
      a.download = `${exportType.key}_${new Date().toISOString().slice(0, 10)}.${format}`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(blobUrl);
    } catch (err) {
      console.error("Export download failed:", err);
    } finally {
      setDownloading(null);
    }
  };

  return (
    <div className="p-4 md:p-6 space-y-6">
      <div>
        <h1 className="text-xl md:text-2xl font-bold text-zinc-100">数据导出</h1>
        <p className="text-sm text-zinc-500">
          导出世界模拟数据的各类视图，支持 JSON / CSV / GraphML 等格式
        </p>
      </div>

      {/* Export Cards */}
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
        {EXPORT_TYPES.map((exportType) => (
          <div
            key={exportType.key}
            className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3"
          >
            <div>
              <h3 className="text-sm font-semibold text-zinc-200">{exportType.label}</h3>
              <p className="text-xs text-zinc-500 mt-1">{exportType.description}</p>
            </div>
            <div className="text-xs text-zinc-400 font-mono">{exportType.path}</div>
            <div className="flex items-center gap-2 flex-wrap">
              {exportType.formats.map((format) => (
                <button
                  key={format}
                  onClick={() => handleDownload(exportType, format)}
                  disabled={downloading === `${exportType.key}-${format}`}
                  className="rounded-lg bg-blue-500/15 px-3 py-1.5 text-xs font-medium text-blue-400 border border-blue-500/30 hover:bg-blue-500/20 transition-colors disabled:opacity-50"
                >
                  {downloading === `${exportType.key}-${format}` ? "下载中..." : `.${format}`}
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>

      {/* API Reference */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-3">
        <h2 className="text-sm font-semibold text-zinc-200">API 参考</h2>
        <div className="text-xs text-zinc-400 space-y-1">
          <p>V1 导出路径: <code className="text-zinc-300">GET /api/v1/export/{"{type}"}?format=json</code></p>
          <p>V2 导出路径: <code className="text-zinc-300">GET /api/v2/export/{"{resource}"}?format=json</code></p>
          <p>支持的 export type: behavior, network, economic, organization, prices</p>
          <p>V2 资源: world, agents/graph, metrics/timeseries, bundle</p>
        </div>
      </div>
    </div>
  );
}
