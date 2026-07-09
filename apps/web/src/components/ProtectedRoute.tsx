import { Navigate, Outlet, useLocation, useNavigate } from "react-router-dom";
import { useEffect } from "react";
import { useAuth } from "../context/AuthProvider";
import { Alert } from "./Alert";
import { PageLoader } from "./Spinner";

export function ProtectedRoute() {
  const { status, user } = useAuth();
  const location = useLocation();
  const navigate = useNavigate();

  useEffect(() => {
    if (status === "unauthenticated") {
      navigate("/login", { replace: true, state: { from: location.pathname } });
    }
  }, [status, navigate, location.pathname]);

  if (status === "loading") {
    return <PageLoader />;
  }

  if (status === "network_error" && !user) {
    return (
      <div className="mx-auto max-w-lg p-8">
        <Alert variant="info">
          Unable to reach the server. Check your connection and try again.
        </Alert>
      </div>
    );
  }

  if (status === "unauthenticated" || !user) {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />;
  }

  return <Outlet />;
}
