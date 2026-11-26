package app.kopru

import java.util.concurrent.atomic.AtomicBoolean

/** Start the transfer only after the PC has accepted the presentation registration. */
internal fun startRegisteredPresentation(
    register: (((Boolean, String?) -> Unit) -> Unit),
    send: () -> Boolean,
    failed: (String?) -> Unit,
) {
    val resolved = AtomicBoolean(false)
    register { ok, error ->
        if (resolved.compareAndSet(false, true)) {
            if (!ok) failed(error) else if (!send()) failed(null)
        }
    }
}
