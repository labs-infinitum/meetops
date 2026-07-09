import { Link } from "react-router-dom";
import { useAuth } from "../context/AuthProvider";
import { Logo } from "../components/Logo";

export function LandingPage() {
  const { status, user } = useAuth();

  return (
    <div className="min-h-screen bg-[#0c1222] text-white">
      <header className="mx-auto flex max-w-6xl items-center justify-between px-6 py-6">
        <Logo className="text-white" />
        <div className="flex items-center gap-3">
          {status === "authenticated" && user ? (
            <Link to="/app" className="btn-primary">
              Open dashboard
            </Link>
          ) : (
            <>
              <Link to="/login" className="btn-ghost text-surface-300 hover:text-white">
                Sign in
              </Link>
              <Link to="/signup" className="btn-primary">
                Get started
              </Link>
            </>
          )}
        </div>
      </header>

      <section className="mx-auto max-w-6xl px-6 pb-24 pt-16">
        <div className="max-w-3xl">
          <p className="mb-4 text-sm font-medium uppercase tracking-wider text-brand-400">
            Google Meet + Google Calendar
          </p>
          <h1 className="font-display text-5xl font-bold leading-tight tracking-tight md:text-6xl">
            Your meetings, recorded and summarized
          </h1>
          <p className="mt-6 text-lg leading-relaxed text-surface-300">
            MeetOps connects to Google Calendar, joins your Google Meet calls with a
            notetaker bot, records the session, and produces structured notes and
            action items. Built for teams that live in Google Workspace — no Outlook or
            Apple Calendar support yet.
          </p>
          <div className="mt-10 flex flex-wrap gap-4">
            <Link to="/signup" className="btn-primary px-6 py-3 text-base">
              Create free account
            </Link>
            <a
              href="#how-it-works"
              className="btn-secondary border-surface-600 bg-transparent px-6 py-3 text-base text-white hover:bg-surface-800"
            >
              How it works
            </a>
          </div>
        </div>
      </section>

      <section id="how-it-works" className="border-t border-surface-800 bg-surface-900/50 py-20">
        <div className="mx-auto max-w-6xl px-6">
          <h2 className="font-display text-3xl font-bold">How MeetOps works</h2>
          <div className="mt-12 grid gap-8 md:grid-cols-3">
            {[
              {
                step: "1",
                title: "Connect Google Calendar",
                desc: "OAuth sync pulls upcoming meetings with Meet links from your Google calendars.",
              },
              {
                step: "2",
                title: "Bot joins & records",
                desc: "MeetOps joins Google Meet at the scheduled time, records audio, and tracks participants.",
              },
              {
                step: "3",
                title: "Notes & todos",
                desc: "After the call, get a markdown summary, TL;DR, and action items you can track in-app.",
              },
            ].map((item) => (
              <div key={item.step} className="rounded-xl border border-surface-700 bg-surface-800/50 p-6">
                <span className="text-sm font-bold text-brand-400">Step {item.step}</span>
                <h3 className="mt-2 font-display text-xl font-semibold">{item.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-surface-400">{item.desc}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="py-20">
        <div className="mx-auto max-w-6xl px-6">
          <h2 className="font-display text-3xl font-bold">What&apos;s included in v1</h2>
          <ul className="mt-8 grid gap-4 md:grid-cols-2">
            {[
              "Google Calendar sync (read-only)",
              "Automatic & ad-hoc Google Meet recording",
              "AI-generated meeting notes in Markdown",
              "Action items with completion tracking",
              "Recording playback via secure URLs",
              "Email notifications for notes & failures",
            ].map((feature) => (
              <li key={feature} className="flex items-start gap-3 text-surface-300">
                <span className="mt-1 text-brand-400">✓</span>
                {feature}
              </li>
            ))}
          </ul>
          <p className="mt-8 text-sm text-surface-500">
            Outlook Calendar and Apple Calendar integrations are on the roadmap — not
            available today.
          </p>
        </div>
      </section>

      <footer className="border-t border-surface-800 py-8 text-center text-sm text-surface-500">
        <p>© {new Date().getFullYear()} MeetOps</p>
      </footer>
    </div>
  );
}
