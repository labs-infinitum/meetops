import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { AppLayout, AuthLayout } from "./components/AppLayout";
import { ProtectedRoute } from "./components/ProtectedRoute";
import { AuthProvider } from "./context/AuthProvider";
import { CalendarsPage } from "./pages/Calendars";
import { CheckEmailPage } from "./pages/CheckEmail";
import { DashboardPage } from "./pages/Dashboard";
import { ForgotPasswordPage } from "./pages/ForgotPassword";
import { LandingPage } from "./pages/Landing";
import { LoginPage } from "./pages/Login";
import { MeetingDetailPage } from "./pages/MeetingDetail";
import { MeetingsPage } from "./pages/Meetings";
import { NoteDetailPage } from "./pages/NoteDetail";
import { NotesPage } from "./pages/Notes";
import { RecordingDetailPage } from "./pages/RecordingDetail";
import { RecordingsPage } from "./pages/Recordings";
import { ResetPasswordPage } from "./pages/ResetPassword";
import { SettingsPage } from "./pages/Settings";
import { SignupPage } from "./pages/Signup";
import { TodosPage } from "./pages/Todos";
import { VerifyEmailPage } from "./pages/VerifyEmail";

export default function App() {
  return (
    <BrowserRouter>
      <AuthProvider>
        <Routes>
          <Route path="/" element={<LandingPage />} />

          <Route element={<AuthLayout />}>
            <Route path="/login" element={<LoginPage />} />
            <Route path="/signup" element={<SignupPage />} />
            <Route path="/forgot-password" element={<ForgotPasswordPage />} />
            <Route path="/reset-password" element={<ResetPasswordPage />} />
            <Route path="/verify-email" element={<VerifyEmailPage />} />
            <Route path="/check-email" element={<CheckEmailPage />} />
          </Route>

          <Route element={<ProtectedRoute />}>
            <Route path="/app" element={<AppLayout />}>
              <Route index element={<DashboardPage />} />
              <Route path="calendars" element={<CalendarsPage />} />
              <Route path="meetings" element={<MeetingsPage />} />
              <Route path="meetings/:id" element={<MeetingDetailPage />} />
              <Route path="recordings" element={<RecordingsPage />} />
              <Route path="recordings/:id" element={<RecordingDetailPage />} />
              <Route path="notes" element={<NotesPage />} />
              <Route path="notes/:id" element={<NoteDetailPage />} />
              <Route path="todos" element={<TodosPage />} />
              <Route path="settings" element={<SettingsPage />} />
            </Route>
          </Route>

          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </AuthProvider>
    </BrowserRouter>
  );
}
