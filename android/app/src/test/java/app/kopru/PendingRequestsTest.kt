package app.kopru

import org.junit.Assert.*
import org.junit.Test

class PendingRequestsTest {
    @Test fun delayedSuccessBeforeBoundedAppBudgetRemovesTimer() {
        val scheduled = mutableMapOf<Runnable, Long>()
        val results = mutableListOf<String>()
        val p = PendingRequests<String>({ r, ms -> scheduled[r] = ms }, { scheduled.remove(it) })
        p.add(1, suiteCommandTimeout("t-downloader", "download.pause", 3), "timeout", results::add)
        assertEquals(25_000L, scheduled.values.single())
        p.complete(1, "success-after-20s")
        assertEquals(listOf("success-after-20s"), results)
        assertTrue(scheduled.isEmpty())
        p.complete(1, "late duplicate")
        assertEquals(1, results.size)
        assertEquals(15_000L, suiteCommandTimeout("t-bridge", "file.present", 3))
        assertEquals(95_000L, suiteCommandTimeout("t-bridge", "suite.update", 3))
        assertEquals(115_000L, suiteCommandTimeout("t-bridge", "suite.check", 5))
        assertEquals(135_000L, suiteCommandTimeout("t-bridge", "suite.update", 100))
    }

    @Test fun timeoutAndDisconnectEachCompleteOnceAndCleanTimers() {
        val scheduled = mutableMapOf<Runnable, Long>()
        val results = mutableListOf<String>()
        val p = PendingRequests<String>({ r, ms -> scheduled[r] = ms }, { scheduled.remove(it) })
        p.add(1, 25_000, "timeout", results::add)
        val timer = scheduled.keys.single()
        timer.run()
        p.complete(1, "late success")
        assertEquals(listOf("timeout"), results)
        assertTrue(scheduled.isEmpty())
        p.add(2, 25_000, "timeout", results::add)
        val cancelledTimer = scheduled.keys.single()
        p.failAll("disconnected")
        cancelledTimer.run()
        p.complete(2, "late success")
        assertEquals(listOf("timeout", "disconnected"), results)
        assertTrue(scheduled.isEmpty())
    }
}
