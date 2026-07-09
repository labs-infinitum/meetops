import { useState } from "react";
import { Link, useLocation } from "react-router-dom";
import { Alert } from "../components/Alert";
import { Spinner } from "../components/Spinner";
import { ApiError, api } from "../lib/api";

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
    <div className="card">
      <h1 className="font-display text-2xl font-bold">Check your email</h1>
      <p className="mt-2 text-sm text-surface-500">
        We sent a verification link to your inbox. Click it to activate your account.
      </p>

      {sent && (
        <Alert variant="success" >
          If that email needs verification, a new link was sent.
        </Alert>
      )}

      <form onSubmit={(e) => void handleResend(e)} className="mt-6 space-y-4">
        {error && <Alert>{error}</Alert>}
        <div>
          <label htmlFor="email" className="label">
            Email
          </label>
          <input
            id="email"
            type="email"
            className="input"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
          />
        </div>
        <button type="submit" disabled={loading} className="btn-secondary w-full">
          {loading ? <Spinner /> : "Resend verification email"}
        </button>
      </form>

      <p className="mt-6 text-center text-sm text-surface-500">
        <Link to="/login" className="font-medium text-brand-600 hover:underline">
          Back to sign in
        </Link>
      </p>
    </div>
  );
}
