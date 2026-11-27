package app.kopru

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OemAutostartTest {
    @Test
    fun detectsMajorAggressiveOems() {
        val xiaomi = OemAutostart.detect("Xiaomi", "Redmi")
        assertNotNull(xiaomi)
        assertEquals(OemAutostart.Kind.XIAOMI, xiaomi!!.kind)
        assertEquals("HyperOS / MIUI", xiaomi.brandLabel)
        assertTrue(xiaomi.components.isNotEmpty())

        assertEquals(OemAutostart.Kind.XIAOMI, OemAutostart.detect("POCO", "POCO")!!.kind)
        assertEquals(OemAutostart.Kind.SAMSUNG, OemAutostart.detect("samsung", "samsung")!!.kind)
        assertEquals("Honor", OemAutostart.detect("HONOR", "HONOR")!!.brandLabel)
        assertEquals("Huawei", OemAutostart.detect("HUAWEI", "HUAWEI")!!.brandLabel)
        assertEquals("OnePlus", OemAutostart.detect("OnePlus", "OnePlus")!!.brandLabel)
        assertEquals("Realme", OemAutostart.detect("realme", "realme")!!.brandLabel)
        assertEquals("OPPO", OemAutostart.detect("OPPO", "OPPO")!!.brandLabel)
        assertEquals("Vivo", OemAutostart.detect("vivo", "vivo")!!.brandLabel)
        assertEquals("ASUS", OemAutostart.detect("asus", "asus")!!.brandLabel)
        assertEquals("Tecno", OemAutostart.detect("TECNO", "TECNO")!!.brandLabel)
        assertEquals("Meizu", OemAutostart.detect("Meizu", "Meizu")!!.brandLabel)
    }

    @Test
    fun returnsNullForStockAndroidDevices() {
        assertNull(OemAutostart.detect("Google", "google"))
        assertNull(OemAutostart.detect("Nothing", "Nothing"))
        assertNull(OemAutostart.detect("motorola", "motorola"))
        assertNull(OemAutostart.detect("Sony", "Sony"))
        assertNull(OemAutostart.detect("", ""))
    }
}
