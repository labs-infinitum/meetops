import { Link, Outlet } from "react-router-dom";
import { Logo } from "./Logo";

export function AuthLayout() {
  return (
    <div className="flex min-h-screen flex-col bg-surface-50 dark:bg-surface-950">
      <header className="px-6 py-5">
        <Logo className="text-surface-900 dark:text-white" />
      </header>
      <main className="flex flex-1 items-center justify-center px-4 pb-12">
        <div className="w-full max-w-md">
          <Outlet />
        </div>
      </main>
      <footer className="px-6 py-4 text-center text-xs text-surface-500">
        <Link to="/" className="hover:text-brand-600">
          Back to home
        </Link>
      </footer>
    </div>
  );
}
