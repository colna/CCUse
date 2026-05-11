import { useCallback, useEffect, useState } from "react";

/**
 * 仪表盘三张图表共同的"挂载即拉、定时轮询"模式。
 *
 * 抽出来是为了：
 * - 收紧依赖项：`load` 只跑一次，挂载时立刻执行，之后按 `intervalMs`
 *   滚动；
 * - 把 loading / error 这两块僵化样板代码留在 hook 中，让 chart 组件
 *   能专注在数据 → 视图的转换。
 *
 * 在仪表盘场景下数据量不大、刷新频率低，没必要引入 SWR 这类客户端
 * 缓存 —— useEffect + setInterval 已经足够。
 */
export interface PollResult<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

export function useTimeseriesPoll<T>(
  load: () => Promise<T>,
  intervalMs: number,
): PollResult<T> {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setData(await load());
      setError(null);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [load]);

  useEffect(() => {
    void refresh();
    const id = setInterval(refresh, intervalMs);
    return () => clearInterval(id);
  }, [refresh, intervalMs]);

  return { data, loading, error, refresh };
}
