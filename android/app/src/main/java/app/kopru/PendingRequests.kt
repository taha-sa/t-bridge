package app.kopru

/** Callback ownership shared by reply, timeout, send failure and disconnect. */
internal class PendingRequests<T>(
    private val schedule: (Runnable, Long) -> Unit,
    private val cancel: (Runnable) -> Unit,
) {
    private data class Entry<T>(val callback: (T) -> Unit, val timeout: Runnable)
    private val entries = HashMap<Long, Entry<T>>()

    fun add(id: Long, timeoutMs: Long, timeoutValue: T, callback: (T) -> Unit) {
        val timeout = Runnable { complete(id, timeoutValue) }
        synchronized(entries) {
            check(id !in entries)
            entries[id] = Entry(callback, timeout)
            schedule(timeout, timeoutMs)
        }
    }

    fun complete(id: Long, value: T) {
        val entry = synchronized(entries) { entries.remove(id)?.also { cancel(it.timeout) } } ?: return
        entry.callback(value)
    }

    fun failAll(value: T) {
        val pending = synchronized(entries) {
            entries.values.toList().also { list -> entries.clear(); list.forEach { cancel(it.timeout) } }
        }
        pending.forEach { runCatching { it.callback(value) } }
    }
}

/** Bridge-local commands do not include another app's launch and T-Link round trip. */
internal fun suiteCommandTimeout(app: String, command: String, appCount: Int): Long = when {
    app != "t-bridge" -> 25_000L
    // Each application can take 12 s to connect and 8 s for a command; retain transport headroom.
    // Cap at 135s for unusually large registries, so the wait remains bounded.
    command == "suite.update" || command == "suite.check" -> appCount.coerceIn(4, 6) * 20_000L + 15_000L
    else -> 15_000L
}
