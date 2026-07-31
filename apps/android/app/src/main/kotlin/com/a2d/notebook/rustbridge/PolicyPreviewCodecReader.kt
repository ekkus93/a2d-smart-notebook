package com.a2d.notebook.rustbridge

import java.nio.ByteBuffer
import java.nio.ByteOrder

internal class PolicyPreviewCodecReader(bytes: ByteArray) {
    private val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)

    fun requireHeader(expectedMagic: String) {
        require(buffer.remaining() >= 8) { "policy preview payload is shorter than its header" }
        val magic = ByteArray(4).also(buffer::get).toString(Charsets.US_ASCII)
        require(magic == expectedMagic) {
            "policy preview magic was $magic instead of $expectedMagic"
        }
        require(readUInt("codec version") == 1L) {
            "unsupported policy preview codec version"
        }
    }

    fun readByte(field: String): Int {
        require(buffer.remaining() >= 1) { "policy preview payload ended before $field" }
        return buffer.get().toInt() and 0xff
    }

    fun readBoolean(field: String): Boolean =
        when (val value = readByte(field)) {
            0 -> false
            1 -> true
            else -> throw IllegalArgumentException("$field must be encoded as 0 or 1, got $value")
        }

    fun readUInt(field: String): Long {
        require(buffer.remaining() >= Int.SIZE_BYTES) {
            "policy preview payload ended before $field"
        }
        return buffer.int.toLong() and 0xffff_ffffL
    }

    fun readInt(field: String): Int {
        val value = readUInt(field)
        require(value <= Int.MAX_VALUE.toLong()) { "$field exceeds the Kotlin Int range" }
        return value.toInt()
    }

    fun readULong(field: String): ULong {
        require(buffer.remaining() >= Long.SIZE_BYTES) {
            "policy preview payload ended before $field"
        }
        return buffer.long.toULong()
    }

    fun readDouble(field: String): Double {
        require(buffer.remaining() >= Double.SIZE_BYTES) {
            "policy preview payload ended before $field"
        }
        return buffer.double
    }

    fun readBytes(field: String, maximumBytes: Int): ByteArray {
        val length = readInt("$field length")
        require(length <= maximumBytes) { "$field length exceeds $maximumBytes bytes" }
        require(length <= buffer.remaining()) { "$field length exceeds the remaining payload" }
        return ByteArray(length).also(buffer::get)
    }

    fun readString(field: String): String =
        readBytes(field, buffer.remaining()).toString(Charsets.UTF_8)

    fun readOptionalDouble(field: String): Double? =
        when (readByte("$field presence")) {
            0 -> null
            1 -> readDouble(field)
            else -> throw IllegalArgumentException("$field presence flag is invalid")
        }

    fun readOptionalULong(field: String): ULong? =
        when (readByte("$field presence")) {
            0 -> null
            1 -> readULong(field)
            else -> throw IllegalArgumentException("$field presence flag is invalid")
        }

    fun requireExhausted() {
        require(!buffer.hasRemaining()) {
            "policy preview payload has ${buffer.remaining()} unexpected trailing bytes"
        }
    }
}
