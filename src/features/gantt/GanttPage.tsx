import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Alert, Badge, Button, Checkbox, Select, Tabs } from "../../components/aizome";
import { PageHeader } from "../../components/PageHeader";
import type { GanttProject, GanttRow } from "../../lib/bindings";
import { formatDate } from "../../lib/date";
import { errorMessage } from "../../lib/errors";
import { useGantt, useProjects } from "../../lib/queries";
import { STATUS_LABEL, STATUS_TONE } from "../todos/todoText";
import {
  addDays,
  DAY_WIDTH,
  dayCenter,
  headerRows,
  LAYOUT,
  layoutRows,
  rangeEnd,
  spanRect,
  startAround,
  step,
  visibleDays,
  visibleRows,
  weekendRuns,
  type LaidOutRow,
  type Range,
  type Scale,
} from "./geometry";
import { rowLabel } from "./rowLabel";

interface GanttPageProps {
  selectedTodoId: number | null;
  onOpenTodo: (id: number) => void;
}

type Row = LaidOutRow<GanttProject, GanttRow>;

const SCALE_TABS: Array<{ id: Scale; label: string }> = [
  { id: "day", label: "日" },
  { id: "week", label: "週" },
  { id: "month", label: "月" },
];

/** ガントチャート (docs/ui-mock/Gantt.dc.html), drawn in SVG. */
export function GanttPage({ selectedTodoId, onOpenTodo }: GanttPageProps) {
  const [scale, setScale] = useState<Scale>("day");
  const [projectId, setProjectId] = useState<number | null>(null);
  const [includeDone, setIncludeDone] = useState(true);
  const [includeCancelled, setIncludeCancelled] = useState(false);
  const projects = useProjects(false);
  const gantt = useGantt({ project_id: projectId, include_done: includeDone, include_cancelled: includeCancelled });
  const today = gantt.data?.today ?? null;

  // Measure the card so the chart shows as many days as fit.
  const scrollRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return undefined;
    const measure = () => setSize({ width: el.clientWidth, height: el.clientHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const chartWidthAvailable = Math.max(0, size.width - LAYOUT.nameWidth - 1);
  const days = visibleDays(chartWidthAvailable || 800, scale);

  // null: put today about a third from the left (initially and after 「今日」).
  const [start, setStart] = useState<string | null>(null);
  const rangeStart = start ?? (today ? startAround(today, days) : null);
  const range: Range | null = rangeStart ? { start: rangeStart, days } : null;

  const [scrollTop, setScrollTop] = useState(0);

  const layout = useMemo(
    () =>
      layoutRows(
        (gantt.data?.projects ?? [])
          .filter((p) => p.rows.length > 0)
          .map((p) => ({
            key: String(p.project.id),
            project: p,
            todos: p.rows.map((r) => ({ key: String(r.item.todo.id), todo: r })),
          })),
      ),
    [gantt.data],
  );
  const bodyHeight = Math.max(0, size.height - LAYOUT.headerHeight);
  const shown = visibleRows(layout.rows, scrollTop, bodyHeight);

  useEffect(() => {
    // Keep the scroll position valid when rows disappear (filters).
    if (scrollRef.current && scrollTop > layout.height) scrollRef.current.scrollTop = 0;
  }, [layout.height, scrollTop]);

  const projectOptions = [
    { value: "all", label: "すべてのプロジェクト" },
    ...(projects.data ?? []).map((p) => ({ value: String(p.project.id), label: p.project.name })),
  ];

  return (
    <>
      <PageHeader
        title="ガントチャート"
        meta={range ? `${formatDate(range.start)} 〜 ${formatDate(rangeEnd(range))}` : undefined}
        actions={
          <div className="gantt-nav">
            <Button
              variant="ghost"
              iconStart="chevron-left"
              aria-label="前の期間へ"
              disabled={!range}
              onClick={() => range && setStart(addDays(range.start, -step(days)))}
            />
            <Button onClick={() => setStart(null)}>今日</Button>
            <Button
              variant="ghost"
              iconStart="chevron-right"
              aria-label="次の期間へ"
              disabled={!range}
              onClick={() => range && setStart(addDays(range.start, step(days)))}
            />
          </div>
        }
      />
      <Tabs
        items={SCALE_TABS}
        value={scale}
        label="表示単位"
        onChange={(id) => {
          setScale(id as Scale);
          setStart(null);
        }}
      />
      <div className="gantt-toolbar">
        <div className="gantt-toolbar__filters">
          <Select
            className="gantt-toolbar__project"
            label="プロジェクト"
            size="sm"
            options={projectOptions}
            value={projectId === null ? "all" : String(projectId)}
            onChange={(e) => setProjectId(e.target.value === "all" ? null : Number(e.target.value))}
          />
          <div className="gantt-toolbar__checks">
            <Checkbox
              label="完了したTODOも表示"
              checked={includeDone}
              onChange={(e) => setIncludeDone(e.target.checked)}
            />
            <Checkbox
              label="中止したTODOも表示"
              checked={includeCancelled}
              onChange={(e) => setIncludeCancelled(e.target.checked)}
            />
          </div>
        </div>
        <Legend />
      </div>
      {gantt.error ? <Alert tone="danger">{errorMessage(gantt.error)}</Alert> : null}

      <div className="gantt">
        <div className="gantt__scroll" ref={scrollRef} onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}>
          {range && today ? (
            <>
              <div className="gantt__header">
                <div className="gantt__name-head">TODO</div>
                <Header range={range} scale={scale} today={today} minWidth={chartWidthAvailable} />
              </div>
              {layout.rows.length === 0 ? (
                <p className="gantt__empty text-muted">
                  表示するTODOはありません。プロジェクトや表示の条件を変えてください。
                </p>
              ) : (
                <div className="gantt__body" style={{ height: layout.height }}>
                  <div className="gantt__names">
                    {shown.map((r) => (
                      <NameCell key={r.key} row={r} selected={r.todo?.item.todo.id === selectedTodoId} onOpen={onOpenTodo} />
                    ))}
                  </div>
                  <Chart
                    range={range}
                    minWidth={chartWidthAvailable}
                    scale={scale}
                    today={today}
                    height={layout.height}
                    rows={shown}
                    selectedTodoId={selectedTodoId}
                    onOpen={onOpenTodo}
                  />
                </div>
              )}
            </>
          ) : null}
        </div>
      </div>
    </>
  );
}

function Legend() {
  return (
    <div className="gantt-legend" aria-label="凡例">
      <span>
        <span aria-hidden="true" className="gantt-legend__planned" />
        予定
      </span>
      <span>
        <span aria-hidden="true" className="gantt-legend__actual" />
        実績
      </span>
      <span>
        <span aria-hidden="true" className="gantt-legend__waiting" />
        待ち
      </span>
      <span>
        <span aria-hidden="true" className="gantt-legend__due" />
        日限
      </span>
      <span>
        <span aria-hidden="true" className="gantt-legend__today" />
        今日
      </span>
    </div>
  );
}

/** SVG width: the days shown, stretched to the card so rows reach its edge. */
function svgWidth(range: Range, scale: Scale, minWidth: number): number {
  return Math.max(range.days * DAY_WIDTH[scale], Math.floor(minWidth));
}

function Header({ range, scale, today, minWidth }: { range: Range; scale: Scale; today: string; minWidth: number }) {
  const w = DAY_WIDTH[scale];
  const width = svgWidth(range, scale, minWidth);
  const { top, bottom } = headerRows(range, scale, today);
  const centered = scale === "day";
  return (
    <svg className="gantt__header-chart" width={width} height={LAYOUT.headerHeight - 1} aria-hidden="true">
      <line className="gantt-grid" x1={0} x2={width} y1={LAYOUT.headerTopRow - 0.5} y2={LAYOUT.headerTopRow - 0.5} />
      {top.map((s, i) => (
        <g key={`t${s.index}`}>
          <text className="gantt-header-top" x={s.index * w + 8} y={16}>
            {s.length * w >= 40 ? s.label : ""}
          </text>
          {i < top.length - 1 ? (
            <line className="gantt-grid" x1={(s.index + s.length) * w - 0.5} x2={(s.index + s.length) * w - 0.5} y1={0} y2={LAYOUT.headerTopRow} />
          ) : null}
        </g>
      ))}
      {bottom.map((s) =>
        centered ? (
          <g key={`b${s.index}`}>
            {s.current ? (
              <rect className="gantt-today-cell" x={s.index * w} y={LAYOUT.headerTopRow} width={w} height={LAYOUT.headerHeight - LAYOUT.headerTopRow - 1} />
            ) : null}
            <text
              className={s.current ? "gantt-header-day is-today" : "gantt-header-day"}
              x={s.index * w + w / 2}
              y={LAYOUT.headerTopRow + 18}
              textAnchor="middle"
            >
              {s.label}
            </text>
          </g>
        ) : (
          <text key={`b${s.index}`} className="gantt-header-day" x={s.index * w + 4} y={LAYOUT.headerTopRow + 18}>
            {s.length * w >= 32 ? s.label : ""}
          </text>
        ),
      )}
    </svg>
  );
}

function NameCell({ row, selected, onOpen }: { row: Row; selected: boolean; onOpen: (id: number) => void }) {
  if (row.kind === "project") {
    return (
      <div className="gantt-name gantt-name--project" style={{ top: row.top, height: row.height }}>
        <span>{row.project.project.name}</span>
        <span className="text-caption">{row.project.rows.length}件</span>
      </div>
    );
  }
  const todo = row.todo!.item.todo;
  return (
    <button
      type="button"
      className={selected ? "gantt-name gantt-name--todo is-selected" : "gantt-name gantt-name--todo"}
      style={{ top: row.top, height: row.height }}
      onClick={() => onOpen(todo.id)}
    >
      <span className="gantt-name__title">{todo.title}</span>
      <Badge tone={STATUS_TONE[todo.status]} dot>
        {STATUS_LABEL[todo.status]}
      </Badge>
    </button>
  );
}

function Chart({
  range,
  minWidth,
  scale,
  today,
  height,
  rows,
  selectedTodoId,
  onOpen,
}: {
  range: Range;
  minWidth: number;
  scale: Scale;
  today: string;
  height: number;
  rows: Row[];
  selectedTodoId: number | null;
  onOpen: (id: number) => void;
}) {
  const w = DAY_WIDTH[scale];
  const width = svgWidth(range, scale, minWidth);
  const todayX = dayCenter(range, today, w);
  return (
    <svg className="gantt__chart" width={width} height={height}>
      {scale !== "month"
        ? weekendRuns(range).map((r) => (
            <rect key={r.index} className="gantt-weekend" x={r.index * w} y={0} width={r.length * w} height={height} />
          ))
        : null}
      {rows.map((r) =>
        r.kind === "project" ? (
          <g key={r.key}>
            <rect className="gantt-project-row" x={0} y={r.top} width={width} height={r.height} />
            <line className="gantt-grid" x1={0} x2={width} y1={r.top + r.height - 0.5} y2={r.top + r.height - 0.5} />
          </g>
        ) : (
          <TodoBars
            key={r.key}
            row={r.todo!}
            top={r.top}
            range={range}
            dayWidth={w}
            width={width}
            selected={r.todo!.item.todo.id === selectedTodoId}
            onOpen={onOpen}
          />
        ),
      )}
      {todayX !== null ? <line className="gantt-today" x1={todayX} x2={todayX} y1={0} y2={height} /> : null}
    </svg>
  );
}

function TodoBars({
  row,
  top,
  range,
  dayWidth,
  width,
  selected,
  onOpen,
}: {
  row: GanttRow;
  top: number;
  range: Range;
  dayWidth: number;
  width: number;
  selected: boolean;
  onOpen: (id: number) => void;
}) {
  const { todo, assessment } = row.item;
  const planned = row.planned ? spanRect(range, row.planned, dayWidth) : null;
  const actual = row.actual ? spanRect(range, row.actual, dayWidth) : null;
  const dueX = todo.due_date ? dayCenter(range, todo.due_date, dayWidth) : null;
  const dueClass =
    assessment.urgency === "overdue" ? "gantt-due is-overdue" : assessment.urgency === "due_today" ? "gantt-due is-today" : "gantt-due";
  const cy = top + LAYOUT.todoRow / 2;
  const half = LAYOUT.diamond / 2;
  return (
    <g role="img" aria-label={rowLabel(row)} className="gantt-row" onClick={() => onOpen(todo.id)}>
      <rect className={selected ? "gantt-row__hit is-selected" : "gantt-row__hit"} x={0} y={top} width={width} height={LAYOUT.todoRow} />
      <line className="gantt-grid" x1={0} x2={width} y1={top + LAYOUT.todoRow - 0.5} y2={top + LAYOUT.todoRow - 0.5} />
      {planned ? (
        <rect className="gantt-planned" x={planned.x} y={top + LAYOUT.plannedTop} width={planned.width} height={LAYOUT.plannedHeight} rx={2} />
      ) : null}
      {actual ? (
        <rect className="gantt-actual" x={actual.x} y={top + LAYOUT.actualTop} width={actual.width} height={LAYOUT.actualHeight} rx={2} />
      ) : null}
      {row.waiting.map((s) => {
        const r = spanRect(range, s, dayWidth);
        return r ? (
          <rect
            key={s.start}
            className="gantt-waiting"
            x={r.x + 1}
            y={top + LAYOUT.actualTop + 1}
            width={Math.max(0, r.width - 2)}
            height={LAYOUT.actualHeight - 2}
            rx={1}
          />
        ) : null;
      })}
      {dueX !== null ? (
        <>
          <rect
            className={dueClass}
            x={dueX - half}
            y={cy - half}
            width={LAYOUT.diamond}
            height={LAYOUT.diamond}
            transform={`rotate(45 ${dueX} ${cy})`}
          />
          {assessment.urgency === "overdue" && assessment.days_until_due !== null ? (
            <text className="gantt-overdue" x={dueX + half + 8} y={cy} dominantBaseline="central">
              {-assessment.days_until_due}日超過
            </text>
          ) : null}
        </>
      ) : null}
    </g>
  );
}
