package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class WebExternalUrlPolicyTest {

    @Test
    fun testValidHttpAndHttpsUrls() {
        val valid1 = WebExternalUrlPolicy.validateAndNormalize("https://www.example.com")
        assertEquals("https://www.example.com", valid1)

        val valid2 = WebExternalUrlPolicy.validateAndNormalize("http://example.org/path/to/page")
        assertEquals("http://example.org/path/to/page", valid2)

        val valid3 = WebExternalUrlPolicy.validateAndNormalize("https://patreon.com/c/nlch/home?ref=1&utm=2#section")
        assertEquals("https://patreon.com/c/nlch/home?ref=1&utm=2#section", valid3)

        // Escaped spaces in path are legitimate
        val validPathEscaped = WebExternalUrlPolicy.validateAndNormalize("https://example.com/my%20path/file%20name.html")
        assertEquals("https://example.com/my%20path/file%20name.html", validPathEscaped)
    }

    @Test
    fun testSchemeAndHostCaseNormalization() {
        val normalizedUpper = WebExternalUrlPolicy.validateAndNormalize("HTTP://EXAMPLE.COM/path")
        assertEquals("http://example.com/path", normalizedUpper)

        val normalizedMixed = WebExternalUrlPolicy.validateAndNormalize("hTtPs://Sub.Domain.Org:8443/API")
        assertEquals("https://sub.domain.org:8443/API", normalizedMixed)
    }

    @Test
    fun testPortBoundaries() {
        assertEquals("http://localhost:8080/", WebExternalUrlPolicy.validateAndNormalize("http://localhost:8080/"))
        assertEquals("https://example.com:1/", WebExternalUrlPolicy.validateAndNormalize("https://example.com:1/"))
        assertEquals("https://example.com:65535/", WebExternalUrlPolicy.validateAndNormalize("https://example.com:65535/"))

        // Invalid ports must fail-closed
        assertNull(WebExternalUrlPolicy.validateAndNormalize("http://example.com:0/"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("http://example.com:65536/"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("http://example.com:-5/"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("http://example.com:port/"))
    }

    @Test
    fun testForbiddenSchemesFailClosed() {
        assertNull(WebExternalUrlPolicy.validateAndNormalize("javascript:alert(1)"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("file:///android_asset/game/index.html"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("file:///etc/passwd"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("content://media/external/images"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("intent://scan/#Intent;scheme=zxing;package=com.google.zxing.client.android;end"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("data:text/html,<h1>exploit</h1>"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("blob:https://example.com/uuid-blob"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("about:blank"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("mailto:support@example.com"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("ftp://ftp.example.com/resource"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("chrome://flags"))
    }

    @Test
    fun testRawWhitespaceRejection() {
        // Raw whitespace must NOT be silently trimmed; input must be rejected entirely
        assertNull(WebExternalUrlPolicy.validateAndNormalize(" https://example.com"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com "))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example .com"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com/path with space"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com\t/path"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com\n/path"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com\r/path"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com\u00A0/path")) // NBSP
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com\u2007/path")) // Figure space
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com/path?q=1\u202F2")) // Narrow NBSP
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com\u3000/path")) // Ideographic space
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com/\uFEFFpath")) // ZWNBSP
    }

    @Test
    fun testControlCharactersRejection() {
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com/\u0000"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com/\u0007"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com/\u001B"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com/\u007F"))
    }

    @Test
    fun testUserinfoAndCredentialSpoofingRejection() {
        assertNull(WebExternalUrlPolicy.validateAndNormalize("http://user:pass@evil.com/"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://admin@example.com/"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com@evil.com/"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://user%40evil.com/"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://evil.com\\@legit.com/"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://evil.com/\\path"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://evil.com%5cpath"))
    }

    @Test
    fun testOpaqueAndMissingAuthorityRejection() {
        assertNull(WebExternalUrlPolicy.validateAndNormalize("http:///path"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://:8080/"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("/relative/path"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("../traversal"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize("example.com"))
        assertNull(WebExternalUrlPolicy.validateAndNormalize(null))
        assertNull(WebExternalUrlPolicy.validateAndNormalize(""))
    }

    @Test
    fun testMaxLengthConstraint() {
        val base = "https://example.com/"
        val padding = "a".repeat(WebExternalUrlPolicy.DEFAULT_MAX_URL_LENGTH - base.length)
        val exact = base + padding
        assertEquals(exact, WebExternalUrlPolicy.validateAndNormalize(exact))

        val overLimit = exact + "b"
        assertNull(WebExternalUrlPolicy.validateAndNormalize(overLimit))

        // Custom max length parameter
        assertNull(WebExternalUrlPolicy.validateAndNormalize("https://example.com", maxLength = 10))
        assertEquals("https://example.com", WebExternalUrlPolicy.validateAndNormalize("https://example.com", maxLength = 50))
    }

    @Test
    fun testPunycodeAndIpv6Literal() {
        // Punycode is valid ASCII domain
        val puny = WebExternalUrlPolicy.validateAndNormalize("https://xn--e1afmkfd.xn--p1ai/path")
        assertEquals("https://xn--e1afmkfd.xn--p1ai/path", puny)

        // IPv6 literal
        val ipv6Loopback = WebExternalUrlPolicy.validateAndNormalize("http://[::1]:8080/index")
        assertEquals("http://[::1]:8080/index", ipv6Loopback)

        val ipv6Address = WebExternalUrlPolicy.validateAndNormalize("https://[2001:db8::1]/test")
        assertEquals("https://[2001:db8::1]/test", ipv6Address)

        // Unencoded IDN in host must fail-closed
        assertNull(WebExternalUrlPolicy.validateAndNormalize("http://münchen.de/"))
    }

    @Test
    fun testAndroidUriParityVerification() {
        val url = "https://example.com:8080/api"
        assertTrue(WebExternalUrlPolicy.verifyAndroidUriParity(url, "https", "example.com", 8080))
        assertTrue(WebExternalUrlPolicy.verifyAndroidUriParity(url, "HTTPS", "EXAMPLE.COM", 8080))

        // Scheme mismatch
        assertFalse(WebExternalUrlPolicy.verifyAndroidUriParity(url, "http", "example.com", 8080))

        // Host mismatch
        assertFalse(WebExternalUrlPolicy.verifyAndroidUriParity(url, "https", "evil.com", 8080))

        // Port mismatch
        assertFalse(WebExternalUrlPolicy.verifyAndroidUriParity(url, "https", "example.com", 9090))

        // IPv6 bracket normalization
        val ipv6Url = "http://[::1]:8080/"
        assertTrue(WebExternalUrlPolicy.verifyAndroidUriParity(ipv6Url, "http", "[::1]", 8080))
        assertTrue(WebExternalUrlPolicy.verifyAndroidUriParity(ipv6Url, "http", "::1", 8080))
    }

    @Test
    fun testSanitizeForLog() {
        val sanitized1 = WebExternalUrlPolicy.sanitizeForLog("https://patreon.com/c/nlch/home?secret_token=12345#auth")
        assertEquals("https://patreon.com", sanitized1)

        val sanitized2 = WebExternalUrlPolicy.sanitizeForLog("http://sub.domain.org:8080/path?query=val")
        assertEquals("http://sub.domain.org:8080", sanitized2)

        val invalidSanitized = WebExternalUrlPolicy.sanitizeForLog("javascript:alert(1)")
        assertEquals("[invalid-url]", invalidSanitized)

        val nullSanitized = WebExternalUrlPolicy.sanitizeForLog(null)
        assertEquals("[null-url]", nullSanitized)
    }
}
