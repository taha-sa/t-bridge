package app.kopru

import java.util.regex.Matcher
import java.util.regex.Pattern

/**
 * Kişisel verilerden arındırma (T-Suite `TSuite.Diagnostics.Scrubber` ile aynı kurallar). En iyi çaba: kullanıcı yine de
 * göndermeden önce önizlemeyi görür.
 */
class Scrubber(private val identities: List<String>, private val maskGuids: Boolean = false) {
    private companion object {
        const val F = Pattern.CASE_INSENSITIVE
        val URL = Pattern.compile("""(?:https?|tbridge|kopru|content|file)://[^\s<>"`]+""", F)
        val CREDS = Pattern.compile("""((?:https?|tbridge|kopru)://)[^/@\s]+@""", F)
        val EMAIL = Pattern.compile("""\b[A-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b""", F)
        val USER_PATH = Pattern.compile("""(\b[A-Z]:[\\/]Users[\\/]|/(?:Users|home)/)[^\\/\r\n"'<>]+""", F)
        val IPV6 = Pattern.compile("""(?<![\w:])(?:[0-9a-f]{0,4}:){2,}[0-9a-f:.]*(?:%[\w.-]+)?""", F)
        val IPV4 = Pattern.compile("""\b(?:\d{1,3}\.){3}\d{1,3}\b""", F)
        val BEARER = Pattern.compile("""\bBearer\s+[^\s,;]+""", F)
        val SECRET_KV = Pattern.compile(
            """\b((?:[\w-]*(?:token|key|auth|secret|password)[\w-]*)["']?\s*[:=]\s*)(?:\[secret\]|"[^"\r\n]*"|'[^'\r\n]*'|Bearer\s+[^\s,;]+|[^\s,;"'&}\]]+)""",
            F,
        )
        val GUID = Pattern.compile("""\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b""", F)
        val TOKEN = Pattern.compile("""(?<![\w+/=-])(?:[0-9a-f]{32,}|[A-Za-z0-9_+/-]{40,}={0,2})(?![\w+/=-])""", F)

        fun isIp(s: String): Boolean {
            // The candidate is numeric IPv6 only: never allow DNS lookups.
            if (':' !in s || s.any { it !in "0123456789abcdefABCDEF:." }) return false
            return runCatching { java.net.InetAddress.getByName(s); true }.getOrDefault(false)
        }

        /** Matcher.replaceAll(Function) yalnız API 34+; bu yüzden elle. */
        fun Pattern.replace(s: String, f: (String) -> String): String {
            val m = matcher(s)
            val sb = StringBuffer()
            while (m.find()) m.appendReplacement(sb, Matcher.quoteReplacement(f(m.group())))
            m.appendTail(sb)
            return sb.toString()
        }
    }

    fun scrub(input: String?): String {
        var s = input ?: ""
        // Kimlik bilgilerini ve sorgu/parça kısmını (zararsız görünenler dahil) at.
        s = URL.replace(s) { u ->
            val end = u.indexOfAny(charArrayOf('?', '#'))
            val cut = if (end >= 0) u.substring(0, end) + "[query]" else u
            CREDS.matcher(cut).replaceAll("$1[credentials]@")
        }
        s = EMAIL.matcher(s).replaceAll("[email]")
        s = USER_PATH.matcher(s).replaceAll("$1[user]")
        for (id in identities.filter { it.isNotBlank() }) {
            s = Pattern.compile("""(?<![\p{L}\p{N}_])""" + Pattern.quote(id) + """(?![\p{L}\p{N}_])""", F).matcher(s).replaceAll("[identity]")
        }
        s = IPV6.replace(s) { v ->
            if (isIp(v.substringBefore('%'))) {
                "[ip]"
            } else {
                val trimmed = v.trimEnd('.', ':')
                if (isIp(trimmed.substringBefore('%'))) "[ip]" + v.substring(trimmed.length) else v
            }
        }
        s = IPV4.matcher(s).replaceAll("[ip]")
        s = BEARER.matcher(s).replaceAll("Bearer [secret]")
        s = SECRET_KV.matcher(s).replaceAll("$1[secret]")
        if (maskGuids) s = GUID.matcher(s).replaceAll("[id]")
        return TOKEN.matcher(s).replaceAll("[token]")
    }
}

