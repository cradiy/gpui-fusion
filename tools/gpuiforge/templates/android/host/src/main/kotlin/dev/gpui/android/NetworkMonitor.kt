package dev.gpui.android

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Handler

/** One default-network callback per session, shared by its Rust subscriptions. */
internal class NetworkMonitor(context: Context, private val handler: Handler,
                              private val deliver: (Long, Int) -> Unit) : AutoCloseable {
    private val context = context.applicationContext
    private val manager = this.context.getSystemService(ConnectivityManager::class.java)
    private val listeners = LinkedHashMap<Long, Int?>()
    private var callback: ConnectivityManager.NetworkCallback? = null
    private var current: Network? = null
    private var state = 0

    private fun checkPermission() {
        check(context.checkSelfPermission(Manifest.permission.ACCESS_NETWORK_STATE) == PackageManager.PERMISSION_GRANTED) {
            "Declare android.permission.ACCESS_NETWORK_STATE in platforms.android.permissions"
        }
    }

    fun snapshot(): Int {
        checkPermission()
        val network = manager.activeNetwork ?: return 0
        return encode(manager.getNetworkCapabilities(network))
    }

    fun subscribe(token: Long) {
        checkPermission()
        if (callback == null) {
            val next = object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    if (callback !== this) return
                    current = network
                    // Capabilities arrive separately; do not query synchronously from callbacks.
                    publish(1)
                }
                override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) {
                    if (callback === this && current == network) publish(encode(capabilities))
                }
                override fun onLost(network: Network) {
                    if (callback !== this || current != network) return
                    current = null
                    publish(0)
                }
            }
            manager.registerDefaultNetworkCallback(next, handler)
            callback = next
            // Register first so a loss during the snapshot cannot be missed.
            try {
                current = manager.activeNetwork
                state = if (current == null) 0 else encode(manager.getNetworkCapabilities(current))
            } catch (error: Exception) {
                close()
                throw error
            }
        }
        listeners[token] = null
        handler.post { emit(token) }
    }

    fun unsubscribe(token: Long) {
        listeners.remove(token)
        if (listeners.isEmpty()) close()
    }

    private fun publish(next: Int) {
        state = next
        // Delivery may remove subscriptions or close the session.
        for (token in listeners.keys.toList()) emit(token)
    }

    private fun emit(token: Long) {
        if (!listeners.containsKey(token) || listeners[token] == state) return
        listeners[token] = state
        deliver(token, state)
    }

    override fun close() {
        val previous = callback
        callback = null
        listeners.clear()
        current = null
        if (previous != null) manager.unregisterNetworkCallback(previous)
    }

    private fun encode(capabilities: NetworkCapabilities?): Int {
        if (capabilities == null) return 1
        return 1 or 2 or
            (if (capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)) 4 else 0) or
            (if (capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)) 0 else 8)
    }
}
