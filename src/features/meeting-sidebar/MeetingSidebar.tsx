import { useCallback, useEffect, useState } from "react";
import {
  meetingsCreate,
  meetingsDelete,
  meetingsList,
  meetingsRename,
  type AppError,
  type Meeting,
} from "../../ipc";
import { errorTitle, friendlyErrorMessage } from "../../shared/lib";
import {
  Button,
  ConfirmDialog,
  DeleteIcon,
  IconButton,
  PlusIcon,
  RenameIcon,
  SidebarIcon,
} from "../../shared/ui";
import styles from "./MeetingSidebar.module.css";

type Props = {
  activeMeetingId: string | null;
  refreshKey: number;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onSelect: (meetingId: string, meeting?: Meeting) => void;
  onNewProject: (meeting: Meeting) => void;
  onDeleted: (meetingId: string) => void;
  onRenamed: (meeting: Meeting) => void;
};

function formatTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) {
    return iso;
  }
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function MeetingSidebar({
  activeMeetingId,
  refreshKey,
  collapsed,
  onToggleCollapsed,
  onSelect,
  onNewProject,
  onDeleted,
  onRenamed,
}: Props) {
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | undefined>();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const [pendingDelete, setPendingDelete] = useState<Meeting | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    try {
      const list = await meetingsList();
      setMeetings(list);
      setError(null);
      setErrorCode(undefined);
    } catch (err) {
      const appErr = err as AppError;
      setError(friendlyErrorMessage(appErr));
      setErrorCode(errorTitle(appErr));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  async function handleNewProject() {
    try {
      const created = await meetingsCreate();
      setError(null);
      setErrorCode(undefined);
      onNewProject(created);
      await load();
    } catch (err) {
      const appErr = err as AppError;
      setError(friendlyErrorMessage(appErr));
      setErrorCode(errorTitle(appErr));
    }
  }

  async function commitRename(meetingId: string) {
    const title = editTitle.trim();
    if (!title) {
      setError("标题不能为空");
      setErrorCode(undefined);
      return;
    }
    try {
      const updated = await meetingsRename(meetingId, title);
      setEditingId(null);
      onRenamed(updated);
      await load();
    } catch (err) {
      const appErr = err as AppError;
      setError(friendlyErrorMessage(appErr));
      setErrorCode(errorTitle(appErr));
    }
  }

  async function confirmDelete() {
    if (!pendingDelete) return;
    const meeting = pendingDelete;
    setDeleting(true);
    try {
      await meetingsDelete(meeting.id);
      setPendingDelete(null);
      onDeleted(meeting.id);
      await load();
    } catch (err) {
      const appErr = err as AppError;
      setError(friendlyErrorMessage(appErr));
      setErrorCode(errorTitle(appErr));
    } finally {
      setDeleting(false);
    }
  }

  if (collapsed) {
    return (
      <aside className={styles.rail}>
        <IconButton label="展开侧边栏" onClick={onToggleCollapsed}>
          <SidebarIcon />
        </IconButton>
        <IconButton label="新建项目" onClick={() => void handleNewProject()}>
          <PlusIcon />
        </IconButton>
      </aside>
    );
  }

  const deleteLabel = pendingDelete
    ? (pendingDelete.title ?? pendingDelete.id)
    : "";

  return (
    <aside className={styles.sidebar}>
      <div className={styles.header}>
        <h2>项目</h2>
        <IconButton
          label="收起侧边栏"
          className={styles.collapse}
          onClick={onToggleCollapsed}
        >
          <SidebarIcon />
        </IconButton>
      </div>
      <div className={styles.toolbar}>
        <Button
          variant="secondary"
          block
          onClick={() => void handleNewProject()}
        >
          <PlusIcon />
          新建项目
        </Button>
      </div>
      {error && (
        <p className={styles.error} role="alert" title={errorCode}>
          {error}
        </p>
      )}
      <ul className={styles.list}>
        {meetings.length === 0 ? (
          <li className={styles.empty}>
            暂无项目。点击上方「新建项目」创建项目后开始录音或导入音频。
          </li>
        ) : (
          meetings.map((meeting) => {
            const active = meeting.id === activeMeetingId;
            const editing = editingId === meeting.id;
            return (
              <li
                key={meeting.id}
                className={active ? styles.itemActive : styles.item}
              >
                {editing ? (
                  <div className={styles.editRow}>
                    <input
                      value={editTitle}
                      onChange={(e) => setEditTitle(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          void commitRename(meeting.id);
                        } else if (e.key === "Escape") {
                          setEditingId(null);
                        }
                      }}
                      aria-label="项目标题"
                      autoFocus
                    />
                    <div className={styles.editActions}>
                      <Button
                        variant="primary"
                        onClick={() => void commitRename(meeting.id)}
                      >
                        保存
                      </Button>
                      <Button
                        variant="ghost"
                        onClick={() => setEditingId(null)}
                      >
                        取消
                      </Button>
                    </div>
                  </div>
                ) : (
                  <>
                    <button
                      type="button"
                      className={styles.selectBtn}
                      onClick={() => onSelect(meeting.id, meeting)}
                    >
                      <span className={styles.title}>
                        {meeting.title ?? meeting.id}
                      </span>
                      <span className={styles.time}>
                        {formatTime(meeting.created_at)}
                      </span>
                    </button>
                    <div className={styles.rowActions}>
                      <IconButton
                        label="重命名"
                        className={styles.rowIcon}
                        onClick={() => {
                          setEditingId(meeting.id);
                          setEditTitle(meeting.title ?? "");
                        }}
                      >
                        <RenameIcon />
                      </IconButton>
                      <IconButton
                        label="删除"
                        className={`${styles.rowIcon} ${styles.rowIconDanger}`}
                        onClick={() => setPendingDelete(meeting)}
                      >
                        <DeleteIcon />
                      </IconButton>
                    </div>
                  </>
                )}
              </li>
            );
          })
        )}
      </ul>
      <ConfirmDialog
        open={pendingDelete !== null}
        title="删除项目"
        description={`确定删除项目「${deleteLabel}」？转写与摘要将一并删除（本地音频文件保留）。`}
        confirmLabel="删除"
        cancelLabel="取消"
        danger
        busy={deleting}
        onConfirm={() => void confirmDelete()}
        onCancel={() => {
          if (!deleting) setPendingDelete(null);
        }}
      />
    </aside>
  );
}
