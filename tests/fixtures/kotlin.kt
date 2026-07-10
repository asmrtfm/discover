package com.example.app

import kotlin.collections.List
import com.example.models.User
import com.example.utils.*
import com.example.network.ApiClient as Client

// Enum class
enum class UserRole { ADMIN, EDITOR, VIEWER }

// Data class
data class UserProfile(val name: String, val age: Int) : Serializable {
    override fun toJson(): String = """{"name":"$name","age":$age}"""
}

// Interface
interface Serializable {
    fun toJson(): String
}

// Sealed class
sealed class Result<out T> {
    data class Success<T>(val data: T) : Result<T>()
    data class Error(val message: String) : Result<Nothing>()
}

// Object (singleton)
object AppConfig {
    const val VERSION = "1.0"
    const val MAX_RETRIES = 3
}

// Companion object inside a class — should not leak as top-level
class Registry {
    companion object {
        fun create(): Registry = Registry()
    }

    fun register(name: String) {}
}

// Abstract class
abstract class BaseRepository<T> {
    abstract fun findById(id: String): T?
    abstract fun save(entity: T)
}

// Annotation class
annotation class Inject

// Top-level functions
fun main() {
    println("Hello")
}

suspend fun fetchUser(id: String): UserProfile {
    return UserProfile("test", 0)
}

fun formatName(first: String, last: String): String {
    return "$first $last"
}

// Top-level property
val MAX_CONNECTIONS = 10

// Typealias
typealias JsonMap = Map<String, Any>
typealias Callback = (String) -> Unit
