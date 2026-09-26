package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.nio.file.Files

class GpackExtractorTest {

    @Test
    fun testExtractArchiveSymlinksToTheMoon() {
        val tempDir = Files.createTempDirectory("extract_symlinks_ttm").toFile()
        try {
            val ttmArchive = File(tempDir, "To the Moon.rgssad")
            ttmArchive.writeBytes(byteArrayOf(0x52, 0x47, 0x53, 0x53, 0x41, 0x44, 0x00, 0x01, 0x12, 0x34))

            GpackExtractor.extractArchiveSymlinks(tempDir, "To the Moon")

            val targetArchive = File(tempDir, "Game.rgssad")
            assertTrue("Expected Game.rgssad alias to exist", targetArchive.exists())
            assertEquals(10, targetArchive.length())
        } finally {
            tempDir.deleteRecursively()
        }
    }

    @Test
    fun testExtractArchiveSymlinksRgss3a() {
        val tempDir = Files.createTempDirectory("extract_symlinks_rgss3a").toFile()
        try {
            val customArchive = File(tempDir, "CustomTitle.rgss3a")
            customArchive.writeBytes(byteArrayOf(0x52, 0x47, 0x53, 0x53, 0x41, 0x44, 0x00, 0x03, 0x56, 0x78))

            GpackExtractor.extractArchiveSymlinks(tempDir, "CustomTitle")

            val targetArchive = File(tempDir, "Game.rgss3a")
            assertTrue("Expected Game.rgss3a alias to exist", targetArchive.exists())
            assertEquals(10, targetArchive.length())
        } finally {
            tempDir.deleteRecursively()
        }
    }

    @Test
    fun testExtractArchiveSymlinksWhenGameAlreadyExists() {
        val tempDir = Files.createTempDirectory("extract_symlinks_exists").toFile()
        try {
            val gameArchive = File(tempDir, "Game.rgssad")
            gameArchive.writeBytes(byteArrayOf(1, 2, 3, 4))

            GpackExtractor.extractArchiveSymlinks(tempDir, "Game")

            assertTrue(gameArchive.exists())
            assertEquals(4, gameArchive.length())
        } finally {
            tempDir.deleteRecursively()
        }
    }

    @Test
    fun testExtractArchiveSymlinksBidirectionalFromGameToExecName() {
        val tempDir = Files.createTempDirectory("extract_symlinks_bidi").toFile()
        try {
            val gameArchive = File(tempDir, "Game.rgssad")
            gameArchive.writeBytes(byteArrayOf(5, 6, 7, 8, 9))

            GpackExtractor.extractArchiveSymlinks(tempDir, "To the Moon")

            val ttmArchive = File(tempDir, "To the Moon.rgssad")
            assertTrue("Expected To the Moon.rgssad alias to exist", ttmArchive.exists())
            assertEquals(5, ttmArchive.length())
        } finally {
            tempDir.deleteRecursively()
        }
    }
}
