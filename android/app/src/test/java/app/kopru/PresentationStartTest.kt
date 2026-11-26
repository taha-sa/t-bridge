package app.kopru

import org.junit.Assert.*
import org.junit.Test

class PresentationStartTest {
    @Test fun delayedRegistrationCannotRaceEvenAnImmediateSmallTransfer() {
        var reply: ((Boolean, String?) -> Unit)? = null
        val events = mutableListOf<String>()
        startRegisteredPresentation({ reply = it; events += "register" }, { events += "small-file-complete"; true }, { events += "failed" })
        assertEquals(listOf("register"), events)
        reply!!(true, null)
        assertEquals(listOf("register", "small-file-complete"), events)
        reply!!(true, null)
        assertEquals(2, events.size)
    }

    @Test fun rejectionOrTimeoutNeverStartsTransfer() {
        for (error in listOf("rejected", "timeout", "disconnected")) {
            var sends = 0
            var failure: String? = null
            startRegisteredPresentation({ it(false, error) }, { sends++; true }, { failure = it })
            assertEquals(0, sends)
            assertEquals(error, failure)
        }
    }

    @Test fun failedTransferIsReportedOnce() {
        var failures = 0
        startRegisteredPresentation({ it(true, null) }, { false }, { failures++ })
        assertEquals(1, failures)
    }
}
