package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class MiniJsonTest {

    @Test
    fun testEscapesAndUnicode() {
        val json = """
            {
                "quote": "hello \"world\"",
                "slash": "a\\b\/c",
                "control": "line1\nline2\ttab\rreturn",
                "unicode": "\u0041\u0042\u4e16\u754c"
            }
        """.trimIndent()

        val obj = MiniJson.parseObjectOrThrow(json)
        assertEquals("hello \"world\"", obj.getString("quote"))
        assertEquals("a\\b/c", obj.getString("slash"))
        assertEquals("line1\nline2\ttab\rreturn", obj.getString("control"))
        assertEquals("AB世界", obj.getString("unicode"))
    }

    @Test
    fun testNegativeAndScientificNumbers() {
        val json = """
            {
                "integer": -42,
                "float": 3.14159,
                "negative_float": -0.005,
                "scientific_pos": 1.23e4,
                "scientific_neg": -5.67E-2
            }
        """.trimIndent()

        val obj = MiniJson.parseObjectOrThrow(json)
        assertEquals(-42, obj.getInt("integer"))
        assertEquals(-42L, obj.getLong("integer"))
        assertEquals(3.14159, (obj.map["float"] as MiniJsonValue.JNumber).value, 1e-6)
        assertEquals(-0.005, (obj.map["negative_float"] as MiniJsonValue.JNumber).value, 1e-6)
        assertEquals(12300.0, (obj.map["scientific_pos"] as MiniJsonValue.JNumber).value, 1e-6)
        assertEquals(-0.0567, (obj.map["scientific_neg"] as MiniJsonValue.JNumber).value, 1e-6)
    }

    @Test
    fun testMalformedLiteralsFailClosed() {
        val badLiterals = listOf(
            """{"val": tru}""",
            """{"val": fals}""",
            """{"val": nul}""",
            """{"val": undefined}"""
        )

        for (bad in badLiterals) {
            try {
                MiniJson.parseOrThrow(bad)
                fail("Expected MiniJsonException for malformed literal: $bad")
            } catch (e: MiniJsonException) {
                // Expected
            }
            assertNull("parse should return null on malformed literal", MiniJson.parse(bad))
        }
    }

    @Test
    fun testTrailingGarbageFailsClosed() {
        val trailing = """{"a": 1} trailing garbage"""
        try {
            MiniJson.parseOrThrow(trailing)
            fail("Expected MiniJsonException for trailing characters")
        } catch (e: MiniJsonException) {
            assertTrue(e.message?.contains("Trailing unexpected characters") == true)
        }
        assertNull(MiniJson.parse(trailing))
    }

    @Test
    fun testDeepNestingExceedingMaxDepthFailsClosed() {
        // Nest depth 65 (exceeding MAX_DEPTH of 64)
        val sb = StringBuilder()
        for (i in 1..65) sb.append("{\"a\":")
        sb.append("1")
        for (i in 1..65) sb.append("}")

        val deeplyNested = sb.toString()
        try {
            MiniJson.parseOrThrow(deeplyNested)
            fail("Expected MiniJsonException for depth > 64")
        } catch (e: MiniJsonException) {
            assertTrue(e.message?.contains("depth exceeded maximum") == true)
        }
        assertNull(MiniJson.parse(deeplyNested))

        // Valid nesting at depth 60 must succeed
        val sbValid = StringBuilder()
        for (i in 1..60) sbValid.append("{\"a\":")
        sbValid.append("1")
        for (i in 1..60) sbValid.append("}")
        val validNested = MiniJson.parseOrThrow(sbValid.toString())
        assertNotNull(validNested)
    }

    @Test
    fun testObjectsAndArraysWithBooleansAndNulls() {
        val json = """
            {
                "flag_true": true,
                "flag_false": false,
                "null_val": null,
                "items": [1, "two", true, null, {"nested": "value"}]
            }
        """.trimIndent()

        val obj = MiniJson.parseObjectOrThrow(json)
        assertEquals(true, obj.getBoolean("flag_true"))
        assertEquals(false, obj.getBoolean("flag_false"))
        assertTrue(obj.map["null_val"] is MiniJsonValue.JNull)

        val arr = obj.getArray("items")
        assertNotNull(arr)
        assertEquals(5, arr!!.items.size)
        assertEquals(1, (arr.items[0] as MiniJsonValue.JNumber).value.toInt())
        assertEquals("two", (arr.items[1] as MiniJsonValue.JString).value)
        assertEquals(true, (arr.items[2] as MiniJsonValue.JBool).value)
        assertTrue(arr.items[3] is MiniJsonValue.JNull)
        val nested = arr.items[4] as MiniJsonValue.JObject
        assertEquals("value", nested.getString("nested"))
    }
}
