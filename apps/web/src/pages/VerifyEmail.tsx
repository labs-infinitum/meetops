import { useEffect, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { Loader2 } from "lucide-react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/context/AuthProvider";
import { ApiError, api } from "@/lib/api";
import type { LoginResponse } from "@/types/api";

export function VerifyEmailPage() {
  const [searchParams] = useSearchParams();
  const token = searchParams.get("token") ?? "";
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const navigate = useNavigate();
  const { setUser } = useAuth();

  useEffect(() => {
    if (!token) {
      setError("Missing verification token.");
      setLoading(false);
      return;
    }

    void (async () => {
      try {
        const data = await api.post<LoginResponse>("/auth/email/verify", { token });
        setUser(data.user);
        navigate("/app", { replace: true });
      } catch (err) {
        if (err instanceof ApiError) {
          setError(err.message);
        } else {
          setError("Unable to connect. Please try again.");
        }
        setLoading(false);
      }
    })();
  }, [token, navigate, setUser]);

  if (loading) {
    return (
      <div className="space-y-4 text-center">
        <h1 className="text-2xl font-semibold tracking-tight">Verifying your email</h1>
        <p className="text-muted-foreground flex items-center justify-center gap-2 text-sm">
          <Loader2 className="size-4 animate-spin" />
          Please wait…
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-6 text-center">
      <div className="space-y-2">
        <h1 className="text-2xl font-semibold tracking-tight">Verification failed</h1>
        {error && <Alert>{error}</Alert>}
        <p className="text-muted-foreground text-sm">
          The link may have expired. Request a new one from the check-email page.
        </p>
      </div>
      <Button asChild>
        <Link to="/check-email">Resend verification</Link>
      </Button>
    </div>
  );
}
