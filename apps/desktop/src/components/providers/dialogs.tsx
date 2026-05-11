import { LoadingOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";

/**
 * 供应商页面用到的两个轻量模态：删除二次确认 + 连接测试失败详情。
 *
 * 为什么不用 antd `Modal`：这里只需要"全屏遮罩 + 居中卡片"，自带的
 * Tailwind 排版加 ARIA 角色已经够覆盖可访问性，且能完全跟着 antd token
 * 之外的 `--app-*` CSS 变量走暗色主题。
 */

interface DeleteDialogProps {
  providerName: string;
  deleting: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function DeleteDialog({
  providerName,
  deleting,
  onConfirm,
  onCancel,
}: DeleteDialogProps) {
  const { t } = useTranslation("providers");
  const { t: tc } = useTranslation("common");
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="delete-provider-dialog-title"
        aria-busy={deleting}
        className="mx-4 w-full max-w-sm rounded-2xl border border-[var(--app-border-secondary)] bg-[var(--app-bg-elevated)] p-6 shadow-xl"
      >
        <h3
          id="delete-provider-dialog-title"
          className="text-base font-semibold"
        >
          {t("delete_title")}
        </h3>
        <p className="mt-2 text-sm text-muted-foreground">
          {t("delete_confirm")}{" "}
          <span className="font-medium text-foreground">{providerName}</span>?{" "}
          {t("delete_undone")}
        </p>
        <div className="mt-5 flex justify-end gap-2">
          <Button type="default" onClick={onCancel} disabled={deleting}>
            {tc("cancel")}
          </Button>
          <Button
            type="primary"
            danger
            onClick={onConfirm}
            disabled={deleting}
            icon={
              deleting ? (
                <LoadingOutlined
                  className="animate-spin"
                  aria-label=""
                  role="presentation"
                />
              ) : undefined
            }
          >
            {deleting ? t("deleting") : tc("delete")}
          </Button>
        </div>
      </div>
    </div>
  );
}

interface ProviderErrorDialogProps {
  title: string;
  providerName: string;
  message: string;
  onClose: () => void;
}

export function ProviderErrorDialog({
  title,
  providerName,
  message,
  onClose,
}: ProviderErrorDialogProps) {
  const { t: tc } = useTranslation("common");
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="provider-error-dialog-title"
        aria-describedby="provider-error-dialog-message"
        className="mx-4 w-full max-w-sm rounded-2xl border border-[var(--app-error-border)] bg-[var(--app-bg-elevated)] p-6 shadow-xl"
      >
        <h3
          id="provider-error-dialog-title"
          className="text-base font-semibold text-foreground"
        >
          {title}
        </h3>
        <p className="mt-2 text-sm text-muted-foreground">{providerName}</p>
        <pre
          id="provider-error-dialog-message"
          className="mt-3 max-h-44 overflow-auto whitespace-pre-wrap rounded-md bg-[var(--app-bg-subtle)] p-3 text-xs text-destructive"
        >
          {message}
        </pre>
        <div className="mt-5 flex justify-end">
          <Button type="default" onClick={onClose}>
            {tc("close")}
          </Button>
        </div>
      </div>
    </div>
  );
}
