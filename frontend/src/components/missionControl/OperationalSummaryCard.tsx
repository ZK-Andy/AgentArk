import { useMemo } from "react";
import { NeuralPanel } from "./NeuralPanel";
import type { Task, TraceSummary } from "../../types";

export type OperationalSummaryCardProps = {
  tasks: Task[];
  traces: TraceSummary[];
  liveRunCount: number;
  requestCount?: number | null;
};

export function OperationalSummaryCard({
  tasks,
  liveRunCount,
  requestCount,
}: OperationalSummaryCardProps) {
  const { completedTodayCount, completedTodayList } = useMemo(() => {
    const todayStr = new Date().toISOString().slice(0, 10);
    const all = Array.isArray(tasks) ? tasks : [];
    const completed = all.filter((task) => {
      const status = String(task?.status ?? "").toLowerCase();
      const isCompleted = status === "completed" || status === "done";
      const isToday = task.created_at ? task.created_at.startsWith(todayStr) : false;
      return isCompleted && isToday;
    });
    return {
      completedTodayCount: completed.length,
      completedTodayList: completed.slice(0, 3),
    };
  }, [tasks]);

  const liveValueCls = liveRunCount > 0 ? "nw-kv-v nw-kv-v--cyan" : "nw-kv-v";
  const requests = requestCount ?? 0;

  return (
    <NeuralPanel title="Operational Summary" tag="TODAY" className="nw-panel--summary">
      <div className="nw-panel-muted">
        Compact view of today&apos;s completion pace, runtime activity, and usage footprint.
      </div>
      <div className="nw-kv-grid nw-kv-grid--3" style={{ marginTop: 12 }}>
        <div className="nw-kv">
          <div className="nw-kv-k">COMPLETED</div>
          <div className="nw-kv-v">{completedTodayCount}</div>
        </div>
        <div className="nw-kv">
          <div className="nw-kv-k">LIVE RUNS</div>
          <div className={liveValueCls}>{liveRunCount}</div>
        </div>
        <div className="nw-kv">
          <div className="nw-kv-k">REQUESTS</div>
          <div className="nw-kv-v">{requests}</div>
        </div>
      </div>
      <div className="nw-panel-muted" style={{ marginTop: 10 }}>
        {completedTodayCount === 0
          ? "No completed tasks have landed yet today."
          : `${completedTodayCount} task(s) completed today.`}
      </div>
      {completedTodayList.length > 0 ? (
        <div className="nw-row-list" style={{ marginTop: 8 }}>
          {completedTodayList.map((task) => (
            <div className="nw-activity-row" key={task.id}>
              <div className="nw-activity-ic">·</div>
              <div className="nw-activity-meta">
                <div className="nw-activity-txt">
                  {String(task.description || "Task completed")}
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : null}
    </NeuralPanel>
  );
}
