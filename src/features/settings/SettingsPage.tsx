import { useState } from "react";
import { Alert, Button, Card, Checkbox, RadioGroup, Switch, Table, TextField } from "../../components/aizome";
import { DateField } from "../../components/DateField";
import { PageHeader } from "../../components/PageHeader";
import type {
  CustomHoliday,
  DayCount,
  Holiday,
  ProjectSummary,
  ReminderFrequency,
  Settings,
  Theme,
} from "../../lib/bindings";
import { formatDate, formatDateWithWeekday, formatTimestampDate, todayYmd } from "../../lib/date";
import { errorMessage } from "../../lib/errors";
import { isComposing } from "../../lib/keys";
import {
  useArchiveProject,
  useHolidays,
  useOpenDataFolder,
  useProjects,
  useSettings,
  useTestNotification,
  useUpdateSettings,
  type SettingsChange,
} from "../../lib/queries";
import { DeleteProjectDialog } from "../projects/DeleteProjectDialog";
import { backupLabel } from "./backupText";
import { ReportTemplateCard } from "./ReportTemplateCard";
import { RestoreDialog } from "./RestoreDialog";

const THEME_OPTIONS: Array<{ value: Theme; label: string }> = [
  { value: "light", label: "ライト" },
  { value: "dark", label: "ダーク" },
  { value: "system", label: "OSの設定に合わせる" },
];

const REMINDER_OPTIONS: Array<{ value: ReminderFrequency; label: string; description: string }> = [
  {
    value: "every_launch",
    label: "起動のたびに表示",
    description: "日限注意が1件でもあれば、起動するたびにダイアログを出します。",
  },
  { value: "once_per_day", label: "1日1回だけ表示", description: "その日最初の起動のときだけダイアログを出します。" },
];

const DAY_COUNT_OPTIONS: Array<{ value: DayCount; label: string; description: string }> = [
  { value: "calendar", label: "暦日で数える", description: "土日や祝日も1日として数えます。" },
  {
    value: "business",
    label: "営業日で数える",
    description:
      "土日、祝日、独自の休日を除いて数えます。休日に当たる日限は、前の営業日に寄せて判定します。待ちの日数は暦日のまま数えます。",
  },
];

/** A development build (`tauri dev`) never registers itself to start at login. */
const DEV_BUILD = import.meta.env.DEV;

/** 設定 (docs/ui-mock/Settings.dc.html). */
export function SettingsPage() {
  const settings = useSettings();
  return (
    <div className="settings-page">
      <PageHeader title="設定" actions={<span className="text-caption">変更はすぐに保存されます</span>} />
      {settings.data ? <ReminderCard key={settings.dataUpdatedAt} settings={settings.data} /> : null}
      {settings.data ? <DayCountCard settings={settings.data} /> : null}
      {settings.data ? <ResidentCard settings={settings.data} /> : null}
      {settings.data ? <ReportTemplateCard template={settings.data.report_template} /> : null}
      <DisplayCard />
      <DataCard />
      <ArchivedProjectsCard />
    </div>
  );
}

/** データ: the data folder and restoring a backup (C-1). */
function DataCard() {
  const openFolder = useOpenDataFolder();
  const [restoring, setRestoring] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  return (
    <Card title="データ" description="データはこのPCの中だけに保存しています。">
      <div className="settings-card__body settings-card__body--tight">
        {openFolder.error ? <Alert tone="danger">{errorMessage(openFolder.error)}</Alert> : null}
        {notice ? (
          <Alert tone="success" onClose={() => setNotice(null)}>
            {notice}
          </Alert>
        ) : null}
        <p className="settings-card__text">1日1回、その日最初の起動時に自動でバックアップし、7日分を残します。</p>
        <div className="settings-card__actions">
          <Button iconStart="folder" disabled={openFolder.isPending} onClick={() => openFolder.mutate()}>
            データフォルダを開く
          </Button>
          <Button
            onClick={() => {
              setNotice(null);
              setRestoring(true);
            }}
          >
            バックアップから復元
          </Button>
        </div>
      </div>
      <RestoreDialog
        open={restoring}
        onClose={() => setRestoring(false)}
        onRestored={(b) => {
          setRestoring(false);
          setNotice(`「${backupLabel(b)}」から復元しました。元に戻すときは「復元する前の状態」を選んで復元してください。`);
        }}
      />
    </Card>
  );
}

/** リマインド: frequency (D-2), 「間近」の日数 and the default follow-up (D-4). */
function ReminderCard({ settings }: { settings: Settings }) {
  const update = useUpdateSettings();
  const [soon, setSoon] = useState(String(settings.soon_days));
  const [followUp, setFollowUp] = useState(String(settings.follow_up_days));
  const [localError, setLocalError] = useState<string | null>(null);

  /** Saves the number fields when they hold a changed whole number. */
  function saveDays() {
    if (!/^\d+$/.test(soon.trim()) || !/^\d+$/.test(followUp.trim())) {
      setLocalError("日数は数字で入力してください。");
      return;
    }
    setLocalError(null);
    const next = { soon_days: Number(soon), follow_up_days: Number(followUp) };
    if (next.soon_days === settings.soon_days && next.follow_up_days === settings.follow_up_days) return;
    update.mutate(next);
  }

  const error = localError ?? (update.error ? errorMessage(update.error) : null);
  const unit = settings.day_count === "business" ? "営業日" : "日";

  return (
    <Card title="リマインド" description="日限注意を知らせる頻度と、区分の基準を決めます。">
      <div className="settings-card__body">
        {error ? <Alert tone="danger">{error}</Alert> : null}
        <RadioGroup
          label="起動時リマインドの表示"
          name="reminder-frequency"
          options={REMINDER_OPTIONS}
          value={settings.reminder_frequency}
          onChange={(value) =>
            update.mutate({ reminder_frequency: value as ReminderFrequency })
          }
        />
        <div className="settings-card__grid">
          <TextField
            label="「間近」とみなす日数"
            inputMode="numeric"
            suffix={`${unit}以内`}
            description="日限がこの日数以内に来るTODOを「間近」に出します。"
            value={soon}
            onChange={(e) => setSoon(e.target.value)}
            onBlur={saveDays}
          />
          <TextField
            label="確認日の初期値"
            inputMode="numeric"
            suffix={`${unit}後`}
            description="「待ち」にした日から数えます。TODOごとに変えられます。"
            value={followUp}
            onChange={(e) => setFollowUp(e.target.value)}
            onBlur={saveDays}
          />
        </div>
      </div>
    </Card>
  );
}

/** 「2026/12/29〜2027/01/03」, or one date. */
function periodLabel(h: CustomHoliday): string {
  return h.start === h.end ? formatDate(h.start) : `${formatDate(h.start)}〜${formatDate(h.end)}`;
}

/** 日数の数え方: 暦日 or 営業日, the custom holidays, and this year's national holidays (v1.1). */
function DayCountCard({ settings }: { settings: Settings }) {
  const update = useUpdateSettings();
  const [name, setName] = useState("");
  const [start, setStart] = useState<string | null>(null);
  const [end, setEnd] = useState<string | null>(null);
  const [yearly, setYearly] = useState(true);
  const [localError, setLocalError] = useState<string | null>(null);
  const year = todayYmd().slice(0, 4);
  const holidays = useHolidays({ start: `${year}-01-01`, end: `${year}-12-31` });
  const national = (holidays.data ?? []).filter((h) => !h.custom);

  function save(change: SettingsChange, onSaved?: () => void) {
    setLocalError(null);
    update.mutate(change, { onSuccess: onSaved });
  }

  function addHoliday() {
    if (!start || !end) {
      setLocalError("独自の休日の開始日と終了日を入力してください。");
      return;
    }
    const added = { name, start, end, yearly };
    save((current) => ({ custom_holidays: [...current.custom_holidays, added] }), () => {
      setName("");
      setStart(null);
      setEnd(null);
    });
  }

  const error = localError ?? (update.error ? errorMessage(update.error) : null);
  const rows = settings.custom_holidays.map((h, i) => ({ id: i, holiday: h }));

  return (
    <Card
      title="日数の数え方"
      description="日限までの日数、「間近」の判定、確認日の初期値を、暦日と営業日のどちらで数えるかを決めます。"
    >
      <div className="settings-card__body">
        {error ? <Alert tone="danger">{error}</Alert> : null}
        <RadioGroup
          label="日数の数え方"
          name="day-count"
          options={DAY_COUNT_OPTIONS}
          value={settings.day_count}
          disabled={update.isPending}
          onChange={(value) => save({ day_count: value as DayCount })}
        />
        <fieldset className="az-field holiday-form" disabled={update.isPending}>
          <legend className="az-field__label">独自の休日</legend>
          {rows.length > 0 ? (
            <Table<{ id: number; holiday: CustomHoliday }>
              rows={rows}
              density="compact"
              columns={[
                { key: "name", header: "名前", render: (r) => r.holiday.name },
                { key: "period", header: "期間", render: (r) => periodLabel(r.holiday) },
                { key: "yearly", header: "くり返し", width: 100, render: (r) => (r.holiday.yearly ? "毎年" : "1回だけ") },
                {
                  key: "actions",
                  header: "操作",
                  align: "right",
                  width: 110,
                  render: (r) => (
                    <Button
                      size="sm"
                      variant="ghost"
                      aria-label={`${r.holiday.name}を削除する`}
                      onClick={() =>
                        save({ custom_holidays: settings.custom_holidays.filter((_, i) => i !== r.id) })
                      }
                    >
                      削除する
                    </Button>
                  ),
                },
              ]}
            />
          ) : (
            <p className="settings-card__text text-muted">独自の休日はまだありません。年末年始などの休みを足せます。</p>
          )}
          <div className="holiday-form__add">
            <TextField
              label="名前"
              placeholder="例: 年末年始"
              maxLength={30}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <DateField
              label="開始日"
              value={start}
              onChange={(v) => {
                setStart(v);
                if (v && !end) setEnd(v);
              }}
            />
            <DateField label="終了日" value={end} onChange={setEnd} />
            <Checkbox label="毎年くり返す" checked={yearly} onChange={(e) => setYearly(e.target.checked)} />
            <Button iconStart="plus" onClick={addHoliday}>
              休日を追加
            </Button>
          </div>
          <p className="az-field__desc">
            1件は31日まで登録できます。毎年くり返す休日は、12月29日〜1月3日のように年をまたげます。祝日と独自の休日は、暦日で数えるときもガントチャートで網掛けします。
          </p>
        </fieldset>
        <div className="az-field">
          <span className="az-field__label">{year}年の祝日</span>
          {holidays.error ? <Alert tone="danger">{errorMessage(holidays.error)}</Alert> : null}
          {national.length > 0 ? (
            <Table<Holiday>
              rowKey="date"
              rows={national}
              density="compact"
              maxHeight={280}
              columns={[
                { key: "date", header: "日付", width: 160, render: (h) => formatDateWithWeekday(h.date) },
                { key: "name", header: "名前", render: (h) => h.name },
              ]}
            />
          ) : holidays.isSuccess ? (
            <p className="settings-card__text text-muted">
              {year}年の祝日は計算していません。祝日を計算するのは2020〜2099年で、ほかの年は土日だけを休みとして数えます。
            </p>
          ) : null}
        </div>
      </div>
    </Card>
  );
}

/** 通知と常駐: OS notifications at fixed times (D-6), the tray and start at login (D-7). */
function ResidentCard({ settings }: { settings: Settings }) {
  const update = useUpdateSettings();
  const testNotification = useTestNotification();
  const [newTime, setNewTime] = useState("");
  const [notice, setNotice] = useState<string | null>(null);

  function save(change: SettingsChange, onSaved?: () => void) {
    setNotice(null);
    testNotification.reset();
    update.mutate(change, { onSuccess: onSaved });
  }

  function addTime() {
    save({ notify_times: [...settings.notify_times, newTime] }, () => setNewTime(""));
  }

  const error = update.error ?? testNotification.error;

  return (
    <Card title="通知と常駐" description="決まった時刻に、日限注意の件数をOSの通知で知らせます。">
      <div className="settings-card__body">
        {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
        {notice ? (
          <Alert tone="success" onClose={() => setNotice(null)}>
            {notice}
          </Alert>
        ) : null}
        <Switch
          label="OSの通知を出す"
          description="日限注意が1件もない時刻には出しません。"
          checked={settings.notify_enabled}
          disabled={update.isPending}
          onChange={(on) => save({ notify_enabled: on })}
        />
        <fieldset className="az-field notify-times" disabled={update.isPending}>
          <legend className="az-field__label">通知の時刻</legend>
          <ul className="notify-times__list">
            {settings.notify_times.map((time) => (
              <li key={time} className="notify-times__item">
                <span className="notify-times__time">{time}</span>
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`${time}を削除する`}
                  onClick={() => save({ notify_times: settings.notify_times.filter((t) => t !== time) })}
                >
                  削除する
                </Button>
              </li>
            ))}
          </ul>
          <div className="notify-times__add">
            <TextField
              aria-label="追加する時刻"
              placeholder="17:30"
              maxLength={5}
              value={newTime}
              onChange={(e) => setNewTime(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !isComposing(e) && newTime.trim() !== "") {
                  e.preventDefault();
                  addTime();
                }
              }}
            />
            <Button iconStart="plus" disabled={newTime.trim() === ""} onClick={addTime}>
              時刻を追加
            </Button>
          </div>
          <p className="az-field__desc">24時間制で、5つまで登録できます。</p>
        </fieldset>
        <div className="settings-card__actions">
          <Button
            disabled={testNotification.isPending}
            onClick={() => {
              setNotice(null);
              update.reset();
              testNotification.mutate(undefined, {
                onSuccess: () =>
                  setNotice("試しの通知を出しました。届かないときは、OSの設定で Higiri の通知が許可されているかを確かめてください。"),
              });
            }}
          >
            通知を試す
          </Button>
        </div>
        <Switch
          label="ウィンドウを閉じてもトレイに残す"
          description="×ボタンで閉じても常駐して、決まった時刻に通知します。オフにすると、×ボタンで終了します。"
          checked={settings.close_to_tray}
          disabled={update.isPending}
          onChange={(on) => save({ close_to_tray: on })}
        />
        <Switch
          label="PCにログインしたときに起動する"
          description={
            DEV_BUILD
              ? "開発ビルドでは使えません。配布版のアプリで設定してください。"
              : "ウィンドウは出さずに、トレイだけで起動します。"
          }
          checked={settings.autostart}
          disabled={DEV_BUILD || update.isPending}
          onChange={(on) => save({ autostart: on })}
        />
      </div>
    </Card>
  );
}

function DisplayCard() {
  const settings = useSettings();
  const update = useUpdateSettings();
  const s = settings.data;

  return (
    <Card title="表示">
      {update.error ? <Alert tone="danger">{errorMessage(update.error)}</Alert> : null}
      {s ? (
        <RadioGroup
          label="配色"
          name="theme"
          orientation="horizontal"
          options={THEME_OPTIONS}
          value={s.theme}
          onChange={(value) => update.mutate({ theme: value as Theme })}
        />
      ) : null}
    </Card>
  );
}

function ArchivedProjectsCard() {
  const projects = useProjects(true);
  const restore = useArchiveProject();
  const [deleteTarget, setDeleteTarget] = useState<ProjectSummary | null>(null);
  const archived = (projects.data ?? []).filter((p) => p.project.archived_at !== null);

  return (
    <Card
      title="アーカイブしたプロジェクト"
      description="一覧・ダッシュボード・ガントから外したプロジェクトです。元に戻すと、また表示されます。"
    >
      {restore.error ? <Alert tone="danger">{errorMessage(restore.error)}</Alert> : null}
      {projects.isSuccess && archived.length === 0 ? (
        <p className="text-muted">
          アーカイブしたプロジェクトはありません。プロジェクトのページの「プロジェクトを編集」からアーカイブできます。
        </p>
      ) : null}
      {archived.length > 0 ? (
        <Table<ProjectSummary>
          rowKey="id"
          rows={archived}
          columns={[
            { key: "name", header: "プロジェクト", render: (p) => p.project.name },
            { key: "todos", header: "TODO", numeric: true, width: 80, render: (p) => `${p.todo_count}件` },
            {
              key: "archived",
              header: "アーカイブした日",
              width: 140,
              render: (p) => (p.project.archived_at ? formatTimestampDate(p.project.archived_at) : ""),
            },
            {
              key: "actions",
              header: "操作",
              align: "right",
              width: 200,
              render: (p) => (
                <div className="table-actions">
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={restore.isPending}
                    onClick={() => restore.mutate({ id: p.project.id, archived: false })}
                  >
                    元に戻す
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setDeleteTarget(p)}>
                    削除する
                  </Button>
                </div>
              ),
            },
          ]}
        />
      ) : null}
      <DeleteProjectDialog
        target={deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onDeleted={() => setDeleteTarget(null)}
      />
    </Card>
  );
}
