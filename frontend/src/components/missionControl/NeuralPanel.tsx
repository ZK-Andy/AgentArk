import type { ReactNode } from "react";
import "./neural-web.css";

export type NeuralPanelTagTone = "default" | "good" | "warn" | "cyan" | "crit";

export type NeuralPanelProps = {
  title: string;
  tag?: string;
  tagTone?: NeuralPanelTagTone;
  alert?: boolean;
  className?: string;
  bodyClassName?: string;
  children?: ReactNode;
  dataTourTarget?: string;
};

export function NeuralPanel({
  title,
  tag,
  tagTone = "default",
  alert = false,
  className,
  bodyClassName,
  children,
  dataTourTarget,
}: NeuralPanelProps) {
  const cls = ["nw-panel", alert ? "nw-panel--alert" : null, className].filter(Boolean).join(" ");
  const tagCls = ["nw-panel-tag", tagTone !== "default" ? `nw-panel-tag--${tagTone}` : null]
    .filter(Boolean)
    .join(" ");
  return (
    <section className={cls} data-tour-target={dataTourTarget}>
      <div className="nw-panel-h">
        <h3 className="nw-panel-title">{title}</h3>
        {tag ? <span className={tagCls}>{tag}</span> : null}
      </div>
      <div className={bodyClassName}>{children}</div>
    </section>
  );
}
