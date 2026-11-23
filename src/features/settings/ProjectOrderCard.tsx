import { useRef, useState } from "react";
import { Alert, Button, Card, Icon, Table } from "../../components/aizome";
import { errorMessage } from "../../lib/errors";
import { useProjects, useReorderProjects } from "../../lib/queries";
import { activeProjects, moved, moveDownLabel, moveUpLabel, PROJECT_ORDER_TEXT, shownOrder } from "./projectOrder";

type Row = { id: number; name: string; first: boolean; last: boolean };

/**
 * プロジェクトの並び (P-4, v0.6.0): the active projects with buttons to move each one up or down. A move is
 * saved at once; the card shows the new order until the save (and the lists read again) is done, and the saved
 * order again when it fails.
 */
export function ProjectOrderCard() {
  const projects = useProjects(false);
  const reorder = useReorderProjects();
  const [pending, setPending] = useState<readonly number[] | null>(null);
  const table = useRef<HTMLDivElement>(null);
  const saved = activeProjects(projects.data ?? []);
  const names = new Map(saved.map((p) => [p.project.id, p.project.name]));
  const order = shownOrder(
    saved.map((p) => p.project.id),
    pending,
  );
  const rows: Row[] = order.map((id, i) => ({
    id,
    name: names.get(id) ?? "",
    first: i === 0,
    last: i === order.length - 1,
  }));
  const error = projects.error ?? reorder.error;

  const button = (id: number, way: "up" | "down") =>
    table.current?.querySelector<HTMLButtonElement>(`[data-move="${id}-${way}"]`) ?? null;

  function move(id: number, delta: -1 | 1) {
    const next = moved(order, id, delta);
    if (next === order) return;
    setPending(next);
    // Only the latest move's callbacks run, after every earlier save.
    reorder.mutate(next, { onSettled: () => setPending(null) });
    // A project moved to the top (or the bottom) can no longer move that way: keep the focus on its row.
    const pressed = button(id, delta < 0 ? "up" : "down");
    const to = next.indexOf(id);
    if (pressed !== null && document.activeElement === pressed && (to === 0 || to === next.length - 1)) {
      requestAnimationFrame(() => button(id, delta < 0 ? "down" : "up")?.focus());
    }
  }

  return (
    <Card title={PROJECT_ORDER_TEXT.title} description={PROJECT_ORDER_TEXT.description}>
      <div className="settings-card__body settings-card__body--tight" ref={table}>
        {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
        {rows.length > 0 ? (
          <Table<Row>
            rows={rows}
            density="compact"
            columns={[
              { key: "name", header: "プロジェクト", render: (r) => r.name },
              {
                key: "move",
                header: PROJECT_ORDER_TEXT.column,
                align: "right",
                width: 120,
                render: (r) => (
                  <span className="project-order__tools">
                    <Button
                      size="sm"
                      variant="ghost"
                      iconStart={<Icon name="chevron-down" size={16} className="icon-up" />}
                      aria-label={moveUpLabel(r.name)}
                      data-move={`${r.id}-up`}
                      disabled={r.first}
                      onClick={() => move(r.id, -1)}
                    />
                    <Button
                      size="sm"
                      variant="ghost"
                      iconStart="chevron-down"
                      aria-label={moveDownLabel(r.name)}
                      data-move={`${r.id}-down`}
                      disabled={r.last}
                      onClick={() => move(r.id, 1)}
                    />
                  </span>
                ),
              },
            ]}
          />
        ) : projects.isSuccess ? (
          <p className="settings-card__text text-muted">{PROJECT_ORDER_TEXT.empty}</p>
        ) : null}
      </div>
    </Card>
  );
}
