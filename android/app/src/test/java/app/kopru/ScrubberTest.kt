package app.kopru

import org.junit.Assert.*
import org.junit.Test

class ScrubberTest {
    private val scrubber = Scrubber(listOf("Alice", "Alice's phone"))

    @Test fun masksCredentialsQueryAndFragment() {
        val text = scrubber.scrub("https://alice:password@example.com/file?ticket=private#secret")
        assertFalse(text.contains("password"))
        assertFalse(text.contains("private"))
        assertTrue(text.contains("[credentials]"))
        assertTrue(text.contains("[query]"))
    }

    @Test fun masksIdentitiesAddressesAndSecrets() {
        val text = scrubber.scrub("Alice alice@example.com C:\\Users\\Alice\\file 192.168.1.9 [2001:db8::1234] token=abcd password=\"hello world\" Authorization: Bearer abcdef")
        for (secret in listOf("Alice", "alice@example.com", "192.168.1.9", "2001:db8::1234", "abcd", "hello world", "abcdef"))
            assertFalse(secret, text.contains(secret))
        assertTrue(text.contains("[ip]"))
        assertTrue(text.contains("[secret]"))
    }

    @Test fun preservesVersionsAndNonAddressText() {
        assertEquals("Version 0.3.2; 12:34:56; Malice", scrubber.scrub("Version 0.3.2; 12:34:56; Malice"))
        assertEquals("", scrubber.scrub(null))
    }

    @Test fun optionalGuidMaskAndLongToken() {
        val id = "12345678-abcd-abcd-abcd-123456789012"
        assertEquals(id, scrubber.scrub(id))
        assertEquals("[id]", Scrubber(emptyList(), true).scrub(id))
        assertEquals("[token]", scrubber.scrub("a".repeat(64)))
    }
}
