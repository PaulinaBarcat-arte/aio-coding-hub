import { invokeTauriOrNull } from "./tauriInvoke";

export type AppAboutInfo = {
  os: string;
  arch: string;
  profile: string;
  app_version: string;
  bundle_type: string | null;
  run_mode: string;
};

export async function appAboutGet() {
  try {
    return await invokeTauriOrNull<AppAboutInfo>("app_about_get");
  } catch {
    return null;
  }
}
