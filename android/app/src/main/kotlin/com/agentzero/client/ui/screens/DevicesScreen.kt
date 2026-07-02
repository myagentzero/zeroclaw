package com.agentzero.client.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import com.agentzero.client.AppContainer
import com.agentzero.client.data.model.PairedDevice
import com.agentzero.client.data.model.ServerConfig
import com.agentzero.client.ui.components.EmptyState
import com.agentzero.client.ui.components.ErrorBanner
import com.agentzero.client.ui.components.LoadingState
import com.agentzero.client.ui.theme.Spacing
import com.agentzero.client.ui.util.formatIsoDateTime
import kotlinx.coroutines.launch

@Composable
fun DevicesScreen(config: ServerConfig, container: AppContainer) {
    var devices by remember { mutableStateOf<List<PairedDevice>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    var refreshing by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var inviteCode by remember { mutableStateOf<String?>(null) }
    var inviting by remember { mutableStateOf(false) }
    var pendingRevoke by remember { mutableStateOf<PairedDevice?>(null) }
    val scope = rememberCoroutineScope()

    fun load(refresh: Boolean = false) {
        scope.launch {
            if (refresh) refreshing = true else loading = true
            error = null
            runCatching { devices = container.gatewayClient.getPairedDevices(config) }
                .onFailure { error = it.message }
            loading = false
            refreshing = false
        }
    }

    LaunchedEffect(config) { load() }

    Column(
        Modifier
            .fillMaxSize()
            .padding(Spacing.md),
        verticalArrangement = Arrangement.spacedBy(Spacing.sm),
    ) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Paired Devices (${devices.size})", style = MaterialTheme.typography.titleMedium)
            Row {
                IconButton(
                    onClick = {
                        inviting = true
                        scope.launch {
                            runCatching {
                                inviteCode = container.gatewayClient.initiateDevicePairing(config)
                            }.onFailure { error = it.message }
                            inviting = false
                        }
                    },
                    enabled = !inviting,
                ) {
                    Icon(Icons.Default.Add, contentDescription = "Add device")
                }
                IconButton(onClick = { load(refresh = true) }, enabled = !refreshing) {
                    Icon(Icons.Default.Refresh, contentDescription = "Refresh")
                }
            }
        }

        error?.let { ErrorBanner(it, onRetry = { load() }) }

        inviteCode?.let { code ->
            Card(
                Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer),
            ) {
                Column(Modifier.padding(Spacing.md)) {
                    Text(
                        "New device pairing code",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                    )
                    Text(
                        code,
                        style = MaterialTheme.typography.headlineMedium,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                        modifier = Modifier.padding(top = Spacing.xs),
                    )
                    Text(
                        "Enter this code on the new device. Valid for this gateway session only.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                        modifier = Modifier.padding(top = Spacing.xs),
                    )
                    TextButton(onClick = { inviteCode = null }) { Text("Dismiss") }
                }
            }
        }

        when {
            loading -> LoadingState()

            devices.isEmpty() -> EmptyState(
                icon = Icons.Default.PhoneAndroid,
                title = "No paired devices found.",
                subtitle = "Tap + to generate an invite code for a new device.",
            )

            else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(Spacing.sm)) {
                items(devices, key = { it.id }) { device ->
                    Card(Modifier.fillMaxWidth()) {
                        Row(Modifier.padding(Spacing.md), verticalAlignment = Alignment.Top) {
                            Box(
                                modifier = Modifier
                                    .size(36.dp)
                                    .background(MaterialTheme.colorScheme.secondaryContainer, CircleShape),
                                contentAlignment = Alignment.Center,
                            ) {
                                Icon(
                                    Icons.Default.PhoneAndroid,
                                    contentDescription = null,
                                    modifier = Modifier.size(18.dp),
                                    tint = MaterialTheme.colorScheme.onSecondaryContainer,
                                )
                            }
                            Column(Modifier.padding(start = Spacing.md).weight(1f)) {
                                Text(
                                    device.deviceName ?: "Unknown",
                                    style = MaterialTheme.typography.titleSmall,
                                )
                                Text(
                                    device.tokenFingerprint,
                                    fontFamily = FontFamily.Monospace,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.padding(top = 2.dp),
                                )
                                Text(
                                    "Paired by: ${device.pairedBy ?: "Unknown"}",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.padding(top = Spacing.xs),
                                )
                                Text(
                                    "Created: ${formatIsoDateTime(device.createdAt)}",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                                TextButton(
                                    onClick = { pendingRevoke = device },
                                    modifier = Modifier.padding(top = Spacing.xs),
                                    contentPadding = PaddingValues(horizontal = 0.dp),
                                ) {
                                    Text("Revoke", color = MaterialTheme.colorScheme.error)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pendingRevoke?.let { device ->
        AlertDialog(
            onDismissRequest = { pendingRevoke = null },
            title = { Text("Revoke device?") },
            text = { Text("Revoke ${device.deviceName ?: device.tokenFingerprint}?") },
            confirmButton = {
                TextButton(onClick = {
                    scope.launch {
                        runCatching { container.gatewayClient.revokePairedDevice(config, device.id) }
                            .onSuccess {
                                devices = devices.filter { it.id != device.id }
                                pendingRevoke = null
                            }
                            .onFailure {
                                error = it.message
                                pendingRevoke = null
                            }
                    }
                }) { Text("Revoke") }
            },
            dismissButton = { TextButton(onClick = { pendingRevoke = null }) { Text("Cancel") } },
        )
    }
}
