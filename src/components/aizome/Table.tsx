import type { ReactElement, ReactNode } from "react";
import { cx } from "./util";

export interface TableColumn<Row> {
  key: string;
  header: ReactNode;
  /** Right-aligned, tabular numerals. */
  numeric?: boolean;
  align?: "left" | "center" | "right";
  width?: number | string;
  render?: (row: Row) => ReactNode;
}

export interface TableProps<Row> {
  columns: TableColumn<Row>[];
  rows: Row[];
  /** Row field used as the React key. Default "id". */
  rowKey?: string;
  selectedKey?: string | number;
  onRowClick?: (row: Row) => void;
  density?: "default" | "compact";
  caption?: ReactNode;
  emptyText?: ReactNode;
  /** Scrolls the body with a sticky header. */
  maxHeight?: number | string;
  className?: string;
}

function field(row: unknown, key: string): unknown {
  return (row as Record<string, unknown>)[key];
}

export function Table<Row>({
  columns = [],
  rows = [],
  rowKey = "id",
  selectedKey,
  onRowClick,
  density,
  caption,
  emptyText,
  maxHeight,
  className,
}: TableProps<Row>): ReactElement {
  return (
    <div
      className={cx("az-table-wrap", className)}
      style={maxHeight ? { maxHeight, overflowY: "auto" } : undefined}
    >
      <table className={cx("az-table", density === "compact" && "az-table--compact")}>
        {caption ? <caption className="az-table__caption">{caption}</caption> : null}
        <thead>
          <tr>
            {columns.map((c) => (
              <th
                key={c.key}
                scope="col"
                className={c.numeric ? "az-num" : undefined}
                style={{ width: c.width, textAlign: c.align }}
              >
                {c.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.length ? (
            rows.map((r, i) => {
              const k = field(r, rowKey);
              const key = k !== undefined ? (k as string | number) : i;
              const selected = selectedKey !== undefined && selectedKey === key;
              return (
                <tr
                  key={key}
                  className={cx(selected && "is-selected", onRowClick && "is-clickable") || undefined}
                  aria-selected={selectedKey !== undefined ? selected : undefined}
                  onClick={onRowClick ? () => onRowClick(r) : undefined}
                >
                  {columns.map((c) => (
                    <td
                      key={c.key}
                      className={c.numeric ? "az-num" : undefined}
                      style={c.align ? { textAlign: c.align } : undefined}
                    >
                      {c.render ? c.render(r) : (field(r, c.key) as ReactNode)}
                    </td>
                  ))}
                </tr>
              );
            })
          ) : (
            <tr>
              <td colSpan={columns.length || 1} className="az-table__empty">
                {emptyText || "データがありません"}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
