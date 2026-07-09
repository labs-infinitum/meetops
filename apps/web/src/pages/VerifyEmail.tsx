import { useEffect, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { Alert } from "../components/Alert";
import { Spinner } from "../components/Spinner";
import { useAuth } from "../context/AuthProvider";
import { ApiError, api } from "../lib/api";
import type { LoginResponse } from "../types/api";

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
      <div className="card flex flex-col items-center py-8">
        <Spinner className="h-8 w-8" />
        <p className="mt-4 text-sm text-surface-500">Verifying your email…</p>
      </div>
    );
  }

  return (
    <div className="card text-center">
      <h1 className="font-display text-2xl font-bold">Verification failed</h1>
      {error && <Alert variant="error">{error}</Alert>}
      <p className="mt-4 text-sm text-surface-500">
        The link may have expired. Request a new one from the check-email page.
      </p>
      <Link to="/check-email" className="btn-primary mt-6 inline-flex">
        Resend verification
      </Link>
    </div>
  );
}
