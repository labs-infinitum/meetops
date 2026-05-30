import { useAuth } from "@/lib/auth-context"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Separator } from "@/components/ui/separator"
import { Switch } from "@/components/ui/switch"
import { PageHeader } from "@/components/shared/PageHeader"
import { toast } from "sonner"

export function SettingsPage() {
  const { session } = useAuth()
  const user = session?.user

  return (
    <div className="space-y-8">
      <PageHeader
        title="Settings"
        description="Manage your account and recording preferences."
      />

      <div className="grid max-w-2xl gap-6">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Profile</CardTitle>
            <CardDescription>Your account information</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="name">Full name</Label>
                <Input id="name" defaultValue={user?.name} />
              </div>
              <div className="space-y-2">
                <Label htmlFor="username">Username</Label>
                <Input id="username" defaultValue={user?.username} />
              </div>
            </div>
            <div className="space-y-2">
              <Label htmlFor="email">Email</Label>
              <Input id="email" type="email" defaultValue={user?.email} />
            </div>
            <Button
              size="sm"
              onClick={() => toast.success("Profile saved (demo)")}
            >
              Save changes
            </Button>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Recording</CardTitle>
            <CardDescription>Bot behavior for auto-recorded meetings</CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium">Auto-record by default</p>
                <p className="text-muted-foreground text-xs">
                  New synced meetings will have auto-record enabled
                </p>
              </div>
              <Switch defaultChecked />
            </div>
            <Separator />
            <div className="space-y-2">
              <Label htmlFor="lead-time">Join lead time (seconds)</Label>
              <Input id="lead-time" type="number" defaultValue={60} className="max-w-32" />
              <p className="text-muted-foreground text-xs">
                How early the bot joins before meeting start
              </p>
            </div>
            <div className="space-y-2">
              <Label htmlFor="bot-name">Bot display name</Label>
              <Input id="bot-name" defaultValue="AI Notetaker" />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Notifications</CardTitle>
            <CardDescription>When to notify you about recordings</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {[
              { label: "Summary ready", desc: "When AI notes are generated", default: true },
              { label: "Recording failed", desc: "When bot cannot join or record", default: true },
              { label: "Daily digest", desc: "Summary of todos and upcoming meetings", default: false },
            ].map((n) => (
              <div key={n.label} className="flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium">{n.label}</p>
                  <p className="text-muted-foreground text-xs">{n.desc}</p>
                </div>
                <Switch defaultChecked={n.default} />
              </div>
            ))}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
