import { useEffect, useState } from "react"
import { Link, useNavigate, useSearchParams } from "react-router-dom"
import { Loader2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { AuthLayout } from "@/components/layout/AppShell"
import { useAuth } from "@/lib/auth-context"
import {
  apiRequestPasswordReset,
  apiResendVerificationEmail,
  apiResetPassword,
  apiVerifyEmail,
  authErrorMessage,
} from "@/lib/auth-api"

export function LoginPage() {
  const { login } = useAuth()
  const navigate = useNavigate()
  const [username, setUsername] = useState("")
  const [password, setPassword] = useState("")
  const [error, setError] = useState("")
  const [emailNotVerified, setEmailNotVerified] = useState(false)
  const [resendEmail, setResendEmail] = useState("")
  const [resendMessage, setResendMessage] = useState("")
  const [resendLoading, setResendLoading] = useState(false)
  const [loading, setLoading] = useState(false)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError("")
    setResendMessage("")
    setEmailNotVerified(false)
    setLoading(true)
    const result = await login(username, password)
    setLoading(false)
    if (result.ok) {
      navigate("/app")
    } else {
      setError(result.error)
      if (result.emailNotVerified) {
        setEmailNotVerified(true)
      }
    }
  }

  async function handleResendVerification() {
    if (!resendEmail.trim()) {
      setResendMessage("Enter the email address you used to sign up.")
      return
    }
    setResendLoading(true)
    setResendMessage("")
    try {
      const message = await apiResendVerificationEmail(resendEmail.trim())
      setResendMessage(message)
    } catch (error) {
      setResendMessage(authErrorMessage(error))
    } finally {
      setResendLoading(false)
    }
  }

  return (
    <AuthLayout>
      <div className="space-y-6">
        <div className="space-y-2 text-center">
          <h1 className="text-2xl font-semibold tracking-tight">Welcome back</h1>
          <p className="text-muted-foreground text-sm">Sign in to your Notetaker account</p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="username">Username</Label>
            <Input
              id="username"
              placeholder="alex"
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label htmlFor="password">Password</Label>
              <Link
                to="/forgot-password"
                className="text-muted-foreground hover:text-foreground text-xs transition-colors"
              >
                Forgot password?
              </Link>
            </div>
            <Input
              id="password"
              type="password"
              placeholder="••••••••"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>

          {error && (
            <p className="text-destructive text-sm">{error}</p>
          )}

          {emailNotVerified && (
            <div className="space-y-2 rounded-lg border border-dashed p-3 text-sm">
              <p className="text-muted-foreground">
                Enter your email to receive a new verification link.
              </p>
              <Input
                type="email"
                placeholder="alex@company.com"
                autoComplete="email"
                value={resendEmail}
                onChange={(e) => setResendEmail(e.target.value)}
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={resendLoading}
                onClick={handleResendVerification}
              >
                {resendLoading && <Loader2 className="animate-spin" />}
                Resend verification email
              </Button>
              {resendMessage && (
                <p className="text-muted-foreground text-xs">{resendMessage}</p>
              )}
            </div>
          )}

          <Button type="submit" className="w-full" disabled={loading}>
            {loading && <Loader2 className="animate-spin" />}
            Sign in
          </Button>
        </form>

        <p className="text-muted-foreground text-center text-sm">
          Don&apos;t have an account?{" "}
          <Link to="/signup" className="text-foreground font-medium hover:underline">
            Sign up
          </Link>
        </p>
      </div>
    </AuthLayout>
  )
}

export function SignupPage() {
  const { signup } = useAuth()
  const navigate = useNavigate()
  const [name, setName] = useState("")
  const [email, setEmail] = useState("")
  const [username, setUsername] = useState("")
  const [password, setPassword] = useState("")
  const [error, setError] = useState("")
  const [loading, setLoading] = useState(false)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError("")
    setLoading(true)
    const result = await signup(name, email, username, password)
    setLoading(false)
    if (result.ok) {
      if (result.verificationRequired) {
        navigate(`/check-email?email=${encodeURIComponent(email)}`)
      } else {
        navigate("/app")
      }
    } else {
      setError(result.error)
    }
  }

  return (
    <AuthLayout>
      <div className="space-y-6">
        <div className="space-y-2 text-center">
          <h1 className="text-2xl font-semibold tracking-tight">Create an account</h1>
          <p className="text-muted-foreground text-sm">Start capturing your meetings for free</p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="name">Full name</Label>
            <Input
              id="name"
              placeholder="Alex Chen"
              autoComplete="name"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="email">Email</Label>
            <Input
              id="email"
              type="email"
              placeholder="alex@company.com"
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="username">Username</Label>
            <Input
              id="username"
              placeholder="alex"
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="password">Password</Label>
            <Input
              id="password"
              type="password"
              placeholder="Min. 8 characters"
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>

          {error && (
            <p className="text-destructive text-sm">{error}</p>
          )}

          <Button type="submit" className="w-full" disabled={loading}>
            {loading && <Loader2 className="animate-spin" />}
            Create account
          </Button>
        </form>

        <p className="text-muted-foreground text-center text-sm">
          Already have an account?{" "}
          <Link to="/login" className="text-foreground font-medium hover:underline">
            Sign in
          </Link>
        </p>
      </div>
    </AuthLayout>
  )
}

export function ForgotPasswordPage() {
  const [email, setEmail] = useState("")
  const [message, setMessage] = useState("")
  const [error, setError] = useState("")
  const [loading, setLoading] = useState(false)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError("")
    setMessage("")
    setLoading(true)
    try {
      const response = await apiRequestPasswordReset(email)
      setMessage(response)
    } catch (err) {
      setError(authErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }

  return (
    <AuthLayout>
      <div className="space-y-6">
        <div className="space-y-2 text-center">
          <h1 className="text-2xl font-semibold tracking-tight">Reset your password</h1>
          <p className="text-muted-foreground text-sm">
            Enter your email and we&apos;ll send you a reset link.
          </p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="email">Email</Label>
            <Input
              id="email"
              type="email"
              placeholder="alex@company.com"
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
          </div>

          {error && <p className="text-destructive text-sm">{error}</p>}
          {message && <p className="text-muted-foreground text-sm">{message}</p>}

          <Button type="submit" className="w-full" disabled={loading}>
            {loading && <Loader2 className="animate-spin" />}
            Send reset link
          </Button>
        </form>

        <p className="text-muted-foreground text-center text-sm">
          <Link to="/login" className="text-foreground font-medium hover:underline">
            Back to sign in
          </Link>
        </p>
      </div>
    </AuthLayout>
  )
}

export function ResetPasswordPage() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const token = searchParams.get("token") ?? ""
  const [password, setPassword] = useState("")
  const [confirmPassword, setConfirmPassword] = useState("")
  const [message, setMessage] = useState("")
  const [error, setError] = useState("")
  const [loading, setLoading] = useState(false)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError("")
    setMessage("")

    if (!token) {
      setError("Missing reset token. Use the link from your email.")
      return
    }
    if (password !== confirmPassword) {
      setError("Passwords do not match.")
      return
    }

    setLoading(true)
    try {
      const response = await apiResetPassword(token, password)
      setMessage(response)
      setTimeout(() => navigate("/login"), 2000)
    } catch (err) {
      setError(authErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }

  return (
    <AuthLayout>
      <div className="space-y-6">
        <div className="space-y-2 text-center">
          <h1 className="text-2xl font-semibold tracking-tight">Choose a new password</h1>
          <p className="text-muted-foreground text-sm">Enter a new password for your account.</p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="password">New password</Label>
            <Input
              id="password"
              type="password"
              placeholder="Min. 8 characters"
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="confirm-password">Confirm password</Label>
            <Input
              id="confirm-password"
              type="password"
              placeholder="Min. 8 characters"
              autoComplete="new-password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              required
            />
          </div>

          {error && <p className="text-destructive text-sm">{error}</p>}
          {message && <p className="text-muted-foreground text-sm">{message}</p>}

          <Button type="submit" className="w-full" disabled={loading || !token}>
            {loading && <Loader2 className="animate-spin" />}
            Update password
          </Button>
        </form>
      </div>
    </AuthLayout>
  )
}

export function VerifyEmailPage() {
  const { setSessionFromVerify } = useAuth()
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const token = searchParams.get("token") ?? ""
  const [error, setError] = useState("")
  const [loading, setLoading] = useState(Boolean(token))

  useEffect(() => {
    if (!token) {
      setError("Missing verification token. Use the link from your email.")
      setLoading(false)
      return
    }

    let cancelled = false
    async function verify() {
      try {
        const session = await apiVerifyEmail(token)
        if (!cancelled) {
          setSessionFromVerify(session)
          navigate("/app", { replace: true })
        }
      } catch (err) {
        if (!cancelled) {
          setError(authErrorMessage(err))
          setLoading(false)
        }
      }
    }

    void verify()
    return () => {
      cancelled = true
    }
  }, [token, navigate, setSessionFromVerify])

  return (
    <AuthLayout>
      <div className="space-y-4 text-center">
        <h1 className="text-2xl font-semibold tracking-tight">Verifying your email</h1>
        {loading && !error && (
          <p className="text-muted-foreground flex items-center justify-center gap-2 text-sm">
            <Loader2 className="size-4 animate-spin" />
            Please wait…
          </p>
        )}
        {error && (
          <>
            <p className="text-destructive text-sm">{error}</p>
            <Link to="/login" className="text-foreground text-sm font-medium hover:underline">
              Back to sign in
            </Link>
          </>
        )}
      </div>
    </AuthLayout>
  )
}

export function CheckEmailPage() {
  const [searchParams] = useSearchParams()
  const email = searchParams.get("email") ?? ""
  const [message, setMessage] = useState("")
  const [error, setError] = useState("")
  const [loading, setLoading] = useState(false)

  async function handleResend() {
    if (!email) {
      setError("Missing email address.")
      return
    }
    setLoading(true)
    setError("")
    setMessage("")
    try {
      const response = await apiResendVerificationEmail(email)
      setMessage(response)
    } catch (err) {
      setError(authErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }

  return (
    <AuthLayout>
      <div className="space-y-6 text-center">
        <div className="space-y-2">
          <h1 className="text-2xl font-semibold tracking-tight">Check your email</h1>
          <p className="text-muted-foreground text-sm">
            We sent a verification link{email ? ` to ${email}` : ""}. Click the link to activate
            your account.
          </p>
        </div>

        {email && (
          <Button type="button" variant="outline" disabled={loading} onClick={handleResend}>
            {loading && <Loader2 className="animate-spin" />}
            Resend verification email
          </Button>
        )}

        {message && <p className="text-muted-foreground text-sm">{message}</p>}
        {error && <p className="text-destructive text-sm">{error}</p>}

        <p className="text-muted-foreground text-sm">
          <Link to="/login" className="text-foreground font-medium hover:underline">
            Back to sign in
          </Link>
        </p>
      </div>
    </AuthLayout>
  )
}
