import { BrowserRouter, Navigate, Route, Routes, useParams } from "react-router-dom"
import { AppShell } from "@/components/layout/AppShell"
import { LandingPage } from "@/pages/LandingPage"
import { LoginPage, SignupPage } from "@/pages/AuthPages"
import { DashboardPage } from "@/pages/DashboardPage"
import { CalendarsPage } from "@/pages/CalendarsPage"
import { MeetingsPage, MeetingDetailPage } from "@/pages/MeetingsPage"
import { RecordingsPage, RecordingDetailPage } from "@/pages/RecordingsPage"
import { NotesPage, NoteDetailPage } from "@/pages/NotesPage"
import { TodosPage } from "@/pages/TodosPage"
import { SettingsPage } from "@/pages/SettingsPage"
import { ProtectedRoute, GuestRoute } from "@/routes/guards"

export function AppRoutes() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<LandingPage />} />
        <Route
          path="/login"
          element={
            <GuestRoute>
              <LoginPage />
            </GuestRoute>
          }
        />
        <Route
          path="/signup"
          element={
            <GuestRoute>
              <SignupPage />
            </GuestRoute>
          }
        />

        <Route
          path="/app"
          element={
            <ProtectedRoute>
              <AppShell />
            </ProtectedRoute>
          }
        >
          <Route index element={<DashboardPage />} />
          <Route path="calendars" element={<CalendarsPage />} />
          <Route path="meetings" element={<MeetingsPage />} />
          <Route path="meetings/:id" element={<MeetingDetailPageWrapper />} />
          <Route path="recordings" element={<RecordingsPage />} />
          <Route path="recordings/:id" element={<RecordingDetailPageWrapper />} />
          <Route path="notes" element={<NotesPage />} />
          <Route path="notes/:id" element={<NoteDetailPageWrapper />} />
          <Route path="todos" element={<TodosPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>

        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}

function MeetingDetailPageWrapper() {
  const { id } = useParams<{ id: string }>()
  if (!id) return <Navigate to="/app/meetings" replace />
  return <MeetingDetailPage meetingId={id} />
}

function RecordingDetailPageWrapper() {
  const { id } = useParams<{ id: string }>()
  if (!id) return <Navigate to="/app/recordings" replace />
  return <RecordingDetailPage recordingId={id} />
}

function NoteDetailPageWrapper() {
  const { id } = useParams<{ id: string }>()
  if (!id) return <Navigate to="/app/notes" replace />
  return <NoteDetailPage noteId={id} />
}
