import type { AppAboutInfo } from "../../services/appAbout";
import { Button } from "../../ui/Button";
import { Card } from "../../ui/Card";
import { SettingsRow } from "../../ui/SettingsRow";

export function SettingsUpdateCard({
  about,
  checkingUpdate,
  checkUpdate,
}: {
  about: AppAboutInfo | null;
  checkingUpdate: boolean;
  checkUpdate: () => Promise<void>;
}) {
  return (
    <Card>
      <div className="mb-4 font-semibold text-slate-900">软件更新</div>
      <div className="divide-y divide-slate-100">
        <SettingsRow label={about?.run_mode === "portable" ? "获取新版本" : "检查更新"}>
          <Button
            onClick={() => void checkUpdate()}
            variant="secondary"
            size="sm"
            disabled={checkingUpdate || !about}
          >
            {checkingUpdate ? "检查中…" : about?.run_mode === "portable" ? "打开" : "检查"}
          </Button>
        </SettingsRow>
      </div>
    </Card>
  );
}
