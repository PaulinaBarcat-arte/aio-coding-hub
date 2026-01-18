import { Outlet } from "react-router-dom";
import { UpdateDialog } from "../components/UpdateDialog";
import { Sidebar } from "../ui/Sidebar";

export function AppLayout() {
  return (
    <div className="min-h-screen bg-slate-50 text-slate-900">
      <div className="flex min-h-screen">
        <Sidebar />

        <div className="min-w-0 flex-1 bg-slate-50">
          <main className="px-6 py-5">
            <Outlet />
          </main>
        </div>
      </div>

      <UpdateDialog />
    </div>
  );
}
