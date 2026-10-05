package io.runtimepack.host

class MiniJsonException(message: String) : Exception(message)

object MiniJson {
    const val MAX_DEPTH = 64
    const val MAX_INPUT_BYTES = 16 * 1024 * 1024

    fun parse(json: String): MiniJsonValue? {
        return try {
            parseOrThrow(json)
        } catch (_: MiniJsonException) {
            null
        }
    }

    fun parseObject(json: String): MiniJsonValue.JObject? {
        return parse(json) as? MiniJsonValue.JObject
    }

    fun parseOrThrow(json: String): MiniJsonValue {
        val bytes = json.toByteArray(Charsets.UTF_8)
        if (bytes.size > MAX_INPUT_BYTES) {
            throw MiniJsonException("Input size (${bytes.size} bytes) exceeds maximum limit of $MAX_INPUT_BYTES bytes")
        }
        val p = Parser(json)
        val value = p.parseValue() ?: throw MiniJsonException("Empty or invalid JSON root")
        p.skipWhitespace()
        if (p.hasMore()) {
            throw MiniJsonException("Trailing unexpected characters after JSON root at index ${p.currentIdx()}")
        }
        return value
    }

    fun parseObjectOrThrow(json: String): MiniJsonValue.JObject {
        val v = parseOrThrow(json)
        return (v as? MiniJsonValue.JObject) ?: throw MiniJsonException("Expected JSON object at root but got ${v.javaClass.simpleName}")
    }

    private class Parser(private val text: String) {
        private var idx = 0
        private val len = text.length
        private var currentDepth = 0

        fun currentIdx(): Int = idx
        fun hasMore(): Boolean = idx < len

        fun parseValue(): MiniJsonValue? {
            skipWhitespace()
            if (idx >= len) return null
            return when (text[idx]) {
                '{' -> parseObject()
                '[' -> parseArray()
                '"' -> parseString()
                't', 'f' -> parseBool()
                'n' -> parseNull()
                '-', in '0'..'9' -> parseNumber()
                else -> throw MiniJsonException("Unexpected character '${text[idx]}' at index $idx")
            }
        }

        private fun parseObject(): MiniJsonValue.JObject {
            if (idx >= len || text[idx] != '{') throw MiniJsonException("Expected '{' at index $idx")
            if (currentDepth >= MAX_DEPTH) {
                throw MiniJsonException("JSON nesting depth exceeded maximum of $MAX_DEPTH at index $idx")
            }
            currentDepth++
            idx++
            val map = mutableMapOf<String, MiniJsonValue>()
            skipWhitespace()
            if (idx < len && text[idx] == '}') {
                idx++
                currentDepth--
                return MiniJsonValue.JObject(map)
            }
            while (idx < len) {
                skipWhitespace()
                if (idx >= len || text[idx] != '"') throw MiniJsonException("Expected string key at index $idx")
                val key = parseRawString() ?: throw MiniJsonException("Unterminated key string at index $idx")
                skipWhitespace()
                if (idx >= len || text[idx] != ':') throw MiniJsonException("Expected ':' after key at index $idx")
                idx++
                val value = parseValue() ?: throw MiniJsonException("Expected value after ':' at index $idx")
                map[key] = value
                skipWhitespace()
                if (idx < len && text[idx] == ',') {
                    idx++
                    continue
                } else if (idx < len && text[idx] == '}') {
                    idx++
                    currentDepth--
                    return MiniJsonValue.JObject(map)
                } else {
                    throw MiniJsonException("Expected ',' or '}' in object at index $idx")
                }
            }
            throw MiniJsonException("Unclosed object starting at depth $currentDepth")
        }

        private fun parseArray(): MiniJsonValue.JArray {
            if (idx >= len || text[idx] != '[') throw MiniJsonException("Expected '[' at index $idx")
            if (currentDepth >= MAX_DEPTH) {
                throw MiniJsonException("JSON nesting depth exceeded maximum of $MAX_DEPTH at index $idx")
            }
            currentDepth++
            idx++
            val list = mutableListOf<MiniJsonValue>()
            skipWhitespace()
            if (idx < len && text[idx] == ']') {
                idx++
                currentDepth--
                return MiniJsonValue.JArray(list)
            }
            while (idx < len) {
                val value = parseValue() ?: throw MiniJsonException("Expected value in array at index $idx")
                list.add(value)
                skipWhitespace()
                if (idx < len && text[idx] == ',') {
                    idx++
                    continue
                } else if (idx < len && text[idx] == ']') {
                    idx++
                    currentDepth--
                    return MiniJsonValue.JArray(list)
                } else {
                    throw MiniJsonException("Expected ',' or ']' in array at index $idx")
                }
            }
            throw MiniJsonException("Unclosed array starting at depth $currentDepth")
        }

        private fun parseRawString(): String? {
            if (idx >= len || text[idx] != '"') return null
            idx++
            val sb = StringBuilder()
            while (idx < len) {
                val c = text[idx++]
                if (c == '"') {
                    return sb.toString()
                } else if (c == '\\') {
                    if (idx >= len) throw MiniJsonException("Unterminated escape sequence at index $idx")
                    when (val esc = text[idx++]) {
                        '"' -> sb.append('"')
                        '\\' -> sb.append('\\')
                        '/' -> sb.append('/')
                        'b' -> sb.append('\b')
                        'f' -> sb.append('\u000C')
                        'n' -> sb.append('\n')
                        'r' -> sb.append('\r')
                        't' -> sb.append('\t')
                        'u' -> {
                            if (idx + 4 > len) throw MiniJsonException("Unterminated unicode escape at index $idx")
                            val hex = text.substring(idx, idx + 4)
                            idx += 4
                            val code = hex.toIntOrNull(16) ?: throw MiniJsonException("Invalid hex in unicode escape '\\u$hex'")
                            sb.append(code.toChar())
                        }
                        else -> throw MiniJsonException("Invalid escape character '\\$esc' at index $idx")
                    }
                } else {
                    sb.append(c)
                }
            }
            throw MiniJsonException("Unterminated string literal at index $idx")
        }

        private fun parseString(): MiniJsonValue.JString {
            val str = parseRawString() ?: throw MiniJsonException("Invalid string at index $idx")
            return MiniJsonValue.JString(str)
        }

        private fun parseNumber(): MiniJsonValue.JNumber {
            val start = idx
            if (idx < len && text[idx] == '-') idx++
            if (idx >= len || text[idx] !in '0'..'9') {
                throw MiniJsonException("Invalid number at index $start")
            }
            while (idx < len && text[idx] in '0'..'9') idx++
            if (idx < len && text[idx] == '.') {
                idx++
                if (idx >= len || text[idx] !in '0'..'9') {
                    throw MiniJsonException("Invalid decimal part in number at index $start")
                }
                while (idx < len && text[idx] in '0'..'9') idx++
            }
            if (idx < len && (text[idx] == 'e' || text[idx] == 'E')) {
                idx++
                if (idx < len && (text[idx] == '+' || text[idx] == '-')) idx++
                if (idx >= len || text[idx] !in '0'..'9') {
                    throw MiniJsonException("Invalid exponent part in number at index $start")
                }
                while (idx < len && text[idx] in '0'..'9') idx++
            }
            val numStr = text.substring(start, idx)
            val d = numStr.toDoubleOrNull() ?: throw MiniJsonException("Malformed number '$numStr' at index $start")
            return MiniJsonValue.JNumber(d)
        }

        private fun parseBool(): MiniJsonValue.JBool {
            return if (text.startsWith("true", idx)) {
                idx += 4
                MiniJsonValue.JBool(true)
            } else if (text.startsWith("false", idx)) {
                idx += 5
                MiniJsonValue.JBool(false)
            } else {
                throw MiniJsonException("Malformed boolean at index $idx")
            }
        }

        private fun parseNull(): MiniJsonValue.JNull {
            return if (text.startsWith("null", idx)) {
                idx += 4
                MiniJsonValue.JNull
            } else {
                throw MiniJsonException("Malformed null at index $idx")
            }
        }

        fun skipWhitespace() {
            while (idx < len) {
                val c = text[idx]
                if (c == ' ' || c == '\t' || c == '\r' || c == '\n') {
                    idx++
                } else {
                    break
                }
            }
        }
    }
}

sealed class MiniJsonValue {
    data class JObject(val map: Map<String, MiniJsonValue>) : MiniJsonValue() {
        fun getString(key: String): String? {
            val v = map[key]
            return if (v is JString) v.value else null
        }

        fun getInt(key: String): Int? {
            val v = map[key]
            return when (v) {
                is JNumber -> v.value.toInt()
                is JString -> v.value.toIntOrNull()
                else -> null
            }
        }

        fun getLong(key: String): Long? {
            val v = map[key]
            return when (v) {
                is JNumber -> v.value.toLong()
                is JString -> v.value.toLongOrNull()
                else -> null
            }
        }

        fun getBoolean(key: String): Boolean? {
            val v = map[key]
            return if (v is JBool) v.value else null
        }

        fun getObject(key: String): JObject? {
            val v = map[key]
            return if (v is JObject) v else null
        }

        fun getArray(key: String): JArray? {
            val v = map[key]
            return if (v is JArray) v else null
        }

        fun keys(): Set<String> = map.keys
    }

    data class JArray(val items: List<MiniJsonValue>) : MiniJsonValue() {
        fun objects(): List<JObject> = items.mapNotNull { it as? JObject }
        fun strings(): List<String> = items.mapNotNull { (it as? JString)?.value }
    }

    data class JString(val value: String) : MiniJsonValue()
    data class JNumber(val value: Double) : MiniJsonValue()
    data class JBool(val value: Boolean) : MiniJsonValue()
    object JNull : MiniJsonValue()
}
