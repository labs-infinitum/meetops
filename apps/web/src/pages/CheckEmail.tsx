import { useState } from "react";
import { Link, useLocation } from "react-router-dom";
import { Loader2 } from "lucide-react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ApiError, api } from "@/lib/api";

export function CheckEmailPage() {
  const location = useLocation();
  const initialEmail = (location.state as { email?: string } | null)?.email ?? "";
  const [email, setEmail] = useState(initialEmail);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleResend(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setLoading(true);
    try {
      await api.post("/auth/email/resend", { email });
      setSent(true);
    } catch (err) {
      if (err instanceof ApiError) {
        setError(err.message);
      } else {
        setError("Unable to connect. Please try again.");
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="space-y-6">
      <div className="space-y-2 text-center">
        <h1 className="text-2xl font-semibold tracking-tight">Check your email</h1>
        <p className="text-muted-foreground text-sm">
          We sent a verification link
          {email ? ` to ${email}` : " to your inbox"}. Click it to activate your account.
        </p>
      </div>

      {sent && (
        <Alert variant="success">If that email needs verification, a new link was sent.</Alert>
      )}

      <form onSubmit={(e) => void handleResend(e)} className="space-y-4">
        {error && <Alert>{error}</Alert>}
        <div className="space-y-2">
          <Label htmlFor="email">Email</Label>
          <Input
            id="email"
            type="email"
            placeholder="alex@company.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
          />
        </div>
        <Button type="submit" variant="outline" className="w-full" disabled={loading}>
          {loading && <Loader2 className="animate-spin" />}
          Resend verification email
        </Button>
      </form>

      <p className="text-muted-foreground text-center text-sm">
        <Link to="/login" className="text-foreground font-medium hover:underline">
          Back to sign in
        </Link>
      </p>
    </div>
  );
}
