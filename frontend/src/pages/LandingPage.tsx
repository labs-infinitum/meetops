import { Link } from "react-router-dom"
import {
  ArrowRight,
  Bot,
  Calendar,
  CheckCircle2,
  FileText,
  ListChecks,
  Sparkles,
  Video,
  Zap,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { MarketingFooter, MarketingNav } from "@/components/layout/MarketingLayout"

const features = [
  {
    icon: Calendar,
    title: "Multi-calendar sync",
    description:
      "Connect Google, Outlook, and Apple calendars. Every meeting with a video link is tracked automatically.",
  },
  {
    icon: Bot,
    title: "Auto-join bot",
    description:
      "A bot joins your Google Meet calls, records audio and video, and handles admission gracefully.",
  },
  {
    icon: Sparkles,
    title: "AI summaries",
    description:
      "Get TL;DR summaries, full markdown notes, and extracted action items within minutes of each call.",
  },
  {
    icon: ListChecks,
    title: "Action items",
    description:
      "Todos are pulled from every meeting and organized in one place — assign, due dates, and completion tracking.",
  },
  {
    icon: FileText,
    title: "Searchable notes",
    description:
      "Browse transcripts and summaries by meeting, tag, or keyword. Never dig through recordings again.",
  },
  {
    icon: Video,
    title: "Recording library",
    description:
      "Access raw recordings with status tracking — queued, recording, completed, or failed with clear errors.",
  },
]

const steps = [
  {
    step: "01",
    title: "Connect your calendars",
    description: "Link one or more calendar accounts. We sync events with video conference links.",
  },
  {
    step: "02",
    title: "Enable auto-record",
    description: "Toggle auto-record per meeting or set defaults. The bot joins 60 seconds before start.",
  },
  {
    step: "03",
    title: "Review AI notes",
    description: "After each call, get summaries, transcripts, and todos delivered to your dashboard.",
  },
]

const plans = [
  {
    name: "Starter",
    price: "Free",
    description: "For individuals getting started",
    features: ["1 calendar", "10 recordings/mo", "AI summaries", "Basic todos"],
    cta: "Get started",
    highlighted: false,
  },
  {
    name: "Pro",
    price: "$19",
    period: "/mo",
    description: "For professionals and small teams",
    features: [
      "Unlimited calendars",
      "Unlimited recordings",
      "Priority transcription",
      "Team sharing",
      "Export to Notion & Slack",
    ],
    cta: "Start free trial",
    highlighted: true,
  },
  {
    name: "Enterprise",
    price: "Custom",
    description: "For organizations at scale",
    features: ["SSO & SAML", "Custom retention", "Dedicated support", "On-prem option"],
    cta: "Contact sales",
    highlighted: false,
  },
]

export function LandingPage() {
  return (
    <div className="min-h-svh">
      <MarketingNav />

      {/* Hero */}
      <section className="relative overflow-hidden">
        <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-violet-500/10 via-transparent to-transparent" />
        <div className="absolute inset-0 bg-[linear-gradient(to_right,var(--border)_1px,transparent_1px),linear-gradient(to_bottom,var(--border)_1px,transparent_1px)] bg-[size:64px_64px] [mask-image:radial-gradient(ellipse_at_center,black_20%,transparent_75%)]" />

        <div className="relative mx-auto max-w-6xl px-4 py-24 sm:px-6 sm:py-32">
          <div className="mx-auto max-w-3xl text-center">
            <Badge variant="secondary" className="mb-6">
              <Zap className="mr-1 size-3" />
              AI-powered meeting intelligence
            </Badge>
            <h1 className="text-4xl font-semibold tracking-tight sm:text-6xl sm:leading-[1.1]">
              Never take meeting notes{" "}
              <span className="bg-gradient-to-r from-violet-600 to-indigo-600 bg-clip-text text-transparent dark:from-violet-400 dark:to-indigo-400">
                again
              </span>
            </h1>
            <p className="text-muted-foreground mx-auto mt-6 max-w-2xl text-lg leading-relaxed">
              Auto-records your Google Meet calls, transcribes conversations, and delivers AI summaries
              with action items — synced from all your calendars.
            </p>
            <div className="mt-10 flex flex-col items-center justify-center gap-3 sm:flex-row">
              <Button size="lg" asChild>
                <Link to="/signup">
                  Start for free
                  <ArrowRight className="size-4" />
                </Link>
              </Button>
              <Button size="lg" variant="outline" asChild>
                <Link to="/login">Sign in</Link>
              </Button>
            </div>
            <p className="text-muted-foreground mt-4 text-sm">
              No credit card required · Connect in under 2 minutes
            </p>
          </div>

          {/* Dashboard preview mockup */}
          <div className="relative mx-auto mt-16 max-w-4xl">
            <div className="from-violet-500/20 to-indigo-500/20 absolute -inset-4 rounded-2xl bg-gradient-to-r blur-2xl" />
            <Card className="relative overflow-hidden shadow-2xl">
              <div className="bg-muted/50 flex items-center gap-2 border-b px-4 py-3">
                <div className="size-3 rounded-full bg-red-400/80" />
                <div className="size-3 rounded-full bg-yellow-400/80" />
                <div className="size-3 rounded-full bg-green-400/80" />
                <span className="text-muted-foreground ml-2 text-xs">app.notetaker.io/dashboard</span>
              </div>
              <CardContent className="grid gap-4 p-6 sm:grid-cols-3">
                {[
                  { label: "Upcoming meetings", value: "4" },
                  { label: "Recordings", value: "12" },
                  { label: "Open todos", value: "5" },
                ].map((stat) => (
                  <div key={stat.label} className="bg-muted/50 rounded-lg p-4">
                    <p className="text-muted-foreground text-xs">{stat.label}</p>
                    <p className="mt-1 text-2xl font-semibold">{stat.value}</p>
                  </div>
                ))}
              </CardContent>
              <div className="border-t px-6 py-4">
                <p className="text-sm font-medium">Latest: Sprint planning — Cycle 14</p>
                <p className="text-muted-foreground mt-1 text-sm">
                  Team committed to shipping onboarding v2 and API v3 by end of cycle...
                </p>
              </div>
            </Card>
          </div>
        </div>
      </section>

      {/* Features */}
      <section id="features" className="border-t py-24">
        <div className="mx-auto max-w-6xl px-4 sm:px-6">
          <div className="mx-auto max-w-2xl text-center">
            <h2 className="text-3xl font-semibold tracking-tight">
              Everything you need after every call
            </h2>
            <p className="text-muted-foreground mt-4">
              From calendar sync to AI notes — one workspace for your entire meeting workflow.
            </p>
          </div>
          <div className="mt-16 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
            {features.map((feature) => (
              <Card key={feature.title} className="border-border/60 transition-shadow hover:shadow-md">
                <CardHeader>
                  <div className="bg-primary/10 text-primary mb-2 flex size-10 items-center justify-center rounded-lg">
                    <feature.icon className="size-5" />
                  </div>
                  <CardTitle className="text-base">{feature.title}</CardTitle>
                  <CardDescription>{feature.description}</CardDescription>
                </CardHeader>
              </Card>
            ))}
          </div>
        </div>
      </section>

      {/* How it works */}
      <section id="how-it-works" className="bg-muted/30 border-t py-24">
        <div className="mx-auto max-w-6xl px-4 sm:px-6">
          <div className="mx-auto max-w-2xl text-center">
            <h2 className="text-3xl font-semibold tracking-tight">How it works</h2>
            <p className="text-muted-foreground mt-4">Three steps to hands-free meeting notes.</p>
          </div>
          <div className="mt-16 grid gap-8 md:grid-cols-3">
            {steps.map((step) => (
              <div key={step.step} className="relative">
                <span className="text-muted-foreground/30 text-5xl font-bold">{step.step}</span>
                <h3 className="mt-4 text-lg font-medium">{step.title}</h3>
                <p className="text-muted-foreground mt-2 text-sm leading-relaxed">{step.description}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Pricing */}
      <section id="pricing" className="border-t py-24">
        <div className="mx-auto max-w-6xl px-4 sm:px-6">
          <div className="mx-auto max-w-2xl text-center">
            <h2 className="text-3xl font-semibold tracking-tight">Simple pricing</h2>
            <p className="text-muted-foreground mt-4">Start free, upgrade when your team grows.</p>
          </div>
          <div className="mt-16 grid gap-6 lg:grid-cols-3">
            {plans.map((plan) => (
              <Card
                key={plan.name}
                className={
                  plan.highlighted
                    ? "border-violet-500/50 shadow-lg shadow-violet-500/10"
                    : "border-border/60"
                }
              >
                <CardHeader>
                  {plan.highlighted && (
                    <Badge className="w-fit">Most popular</Badge>
                  )}
                  <CardTitle>{plan.name}</CardTitle>
                  <CardDescription>{plan.description}</CardDescription>
                  <div className="mt-4">
                    <span className="text-4xl font-semibold">{plan.price}</span>
                    {plan.period && (
                      <span className="text-muted-foreground text-sm">{plan.period}</span>
                    )}
                  </div>
                </CardHeader>
                <CardContent className="space-y-4">
                  <ul className="space-y-2">
                    {plan.features.map((f) => (
                      <li key={f} className="flex items-center gap-2 text-sm">
                        <CheckCircle2 className="text-primary size-4 shrink-0" />
                        {f}
                      </li>
                    ))}
                  </ul>
                  <Button
                    className="w-full"
                    variant={plan.highlighted ? "default" : "outline"}
                    asChild
                  >
                    <Link to="/signup">{plan.cta}</Link>
                  </Button>
                </CardContent>
              </Card>
            ))}
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="border-t py-24">
        <div className="mx-auto max-w-6xl px-4 sm:px-6">
          <Card className="overflow-hidden border-0 bg-zinc-950 text-white">
            <CardContent className="relative px-8 py-16 text-center sm:px-16">
              <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,_var(--tw-gradient-stops))] from-violet-600/20 via-transparent to-transparent" />
              <div className="relative">
                <h2 className="text-3xl font-semibold tracking-tight">
                  Ready to reclaim your meeting time?
                </h2>
                <p className="mx-auto mt-4 max-w-lg text-white/60">
                  Join teams who capture every conversation and never miss an action item.
                </p>
                <Button size="lg" className="mt-8" asChild>
                  <Link to="/signup">
                    Get started free
                    <ArrowRight className="size-4" />
                  </Link>
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      </section>

      <MarketingFooter />
    </div>
  )
}
