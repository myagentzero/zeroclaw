package com.agentzero.client.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Block
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Public
import androidx.compose.material.icons.filled.Shield
import androidx.compose.material.icons.filled.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import com.agentzero.client.AppContainer
import com.agentzero.client.data.model.EstopStatus
import com.agentzero.client.data.model.ServerConfig
import com.agentzero.client.ui.components.EmptyState
import com.agentzero.client.ui.components.ErrorBanner
import com.agentzero.client.ui.theme.Spacing
import kotlinx.coroutines.launch

private data class ResumeArgs(
    val network: Boolean = false,
    val domains: List<String> = emptyList(),
    val tools: List<String> = emptyList(),
)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun EstopScreen(config: ServerConfig, container: AppContainer) {
    var status by remember { mutableStateOf<EstopStatus?>(null) }
    var loading by remember { mutableStateOf(true) }
    var disabled by remember { mutableStateOf(false) }
    var loadError by remember { mutableStateOf<String?>(null) }

    var busy by remember { mutableStateOf(false) }
    var actionError by remember { mutableStateOf<String?>(null) }
    var actionMessage by remember { mutableStateOf<String?>(null) }

    var domainInput by remember { mutableStateOf("") }
    var toolInput by remember { mutableStateOf("") }
    var otpPrompt by remember { mutableStateOf<ResumeArgs?>(null) }
    var otpCode by remember { mutableStateOf("") }

    val scope = rememberCoroutineScope()

    fun refresh() {
        scope.launch {
            loading = true
            runCatching { container.gatewayClient.getEstopStatus(config) }
                .onSuccess {
                    status = it
                    disabled = false
                    loadError = null
                }
                .onFailure { err ->
                    val message = err.message ?: "Failed to load estop status"
                    if (message.contains("disabled", ignoreCase = true)) {
                        disabled = true
                    } else {
                        loadError = message
                    }
                }
            loading = false
        }
    }

    fun engage(level: String, domains: List<String> = emptyList(), tools: List<String> = emptyList()) {
        scope.launch {
            busy = true
            actionError = null
            actionMessage = null
            runCatching { container.gatewayClient.engageEstop(config, level, domains, tools) }
                .onSuccess {
                    status = it
                    actionMessage = "Emergency stop engaged."
                }
                .onFailure { actionError = it.message ?: "Failed to engage emergency stop" }
            busy = false
        }
    }

    fun resume(args: ResumeArgs, otp: String? = null) {
        scope.launch {
            busy = true
            actionError = null
            actionMessage = null
            runCatching {
                container.gatewayClient.resumeEstop(config, args.network, args.domains, args.tools, otp)
            }
                .onSuccess {
                    status = it
                    actionMessage = "Resume completed."
                    otpPrompt = null
                    otpCode = ""
                }
                .onFailure { actionError = it.message ?: "Failed to resume" }
            busy = false
        }
    }

    fun handleResume(args: ResumeArgs) {
        if (status?.requireOtpToResume == true) {
            otpPrompt = args
            otpCode = ""
        } else {
            resume(args)
        }
    }

    LaunchedEffect(config) { refresh() }

    when {
        loading && status == null -> Column(
            Modifier.fillMaxSize(),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            CircularProgressIndicator()
        }

        disabled -> EmptyState(
            icon = Icons.Default.Shield,
            title = "Emergency stop is disabled",
            subtitle = "Set [security.estop] enabled = true in config.toml and restart AgentZero " +
                "to enable engage/resume controls here.",
        )

        loadError != null && status == null -> ErrorBanner(
            "Failed to load emergency stop status: $loadError",
            onRetry = { refresh() },
        )

        else -> {
            val current = status
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
                    Text("Emergency Stop", style = MaterialTheme.typography.titleMedium)
                    EstopBadge(active = current?.isEngaged == true, activeLabel = "Engaged", inactiveLabel = "Clear")
                }

                actionError?.let { ErrorBanner(it) }
                if (actionMessage != null) {
                    Surface(
                        color = MaterialTheme.colorScheme.primaryContainer,
                        shape = MaterialTheme.shapes.medium,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(
                            actionMessage!!,
                            modifier = Modifier.padding(Spacing.sm),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onPrimaryContainer,
                        )
                    }
                }

                LazyColumn(verticalArrangement = Arrangement.spacedBy(Spacing.md)) {
                    item {
                        Card(Modifier.fillMaxWidth()) {
                            Column(Modifier.padding(Spacing.md), verticalArrangement = Arrangement.spacedBy(Spacing.sm)) {
                                Text("Engage", style = MaterialTheme.typography.labelLarge)
                                Row(horizontalArrangement = Arrangement.spacedBy(Spacing.sm)) {
                                    Button(
                                        onClick = { engage("kill-all") },
                                        enabled = !busy,
                                        colors = ButtonDefaults.buttonColors(
                                            containerColor = MaterialTheme.colorScheme.error,
                                        ),
                                    ) { Text("Kill All") }
                                    OutlinedButton(
                                        onClick = { engage("network-kill") },
                                        enabled = !busy,
                                    ) { Text("Network Kill") }
                                }

                                OutlinedTextField(
                                    value = domainInput,
                                    onValueChange = { domainInput = it },
                                    label = { Text("Block domain(s), comma-separated") },
                                    placeholder = { Text("*.example.com, other.com") },
                                    singleLine = true,
                                    modifier = Modifier.fillMaxWidth(),
                                )
                                OutlinedButton(
                                    onClick = {
                                        val domains = domainInput.split(",").map { it.trim() }.filter { it.isNotEmpty() }
                                        engage("domain-block", domains = domains)
                                        domainInput = ""
                                    },
                                    enabled = !busy && domainInput.isNotBlank(),
                                ) { Text("Block Domains") }

                                OutlinedTextField(
                                    value = toolInput,
                                    onValueChange = { toolInput = it },
                                    label = { Text("Freeze tool(s), comma-separated") },
                                    placeholder = { Text("shell, file_write") },
                                    singleLine = true,
                                    modifier = Modifier.fillMaxWidth(),
                                )
                                OutlinedButton(
                                    onClick = {
                                        val tools = toolInput.split(",").map { it.trim() }.filter { it.isNotEmpty() }
                                        engage("tool-freeze", tools = tools)
                                        toolInput = ""
                                    },
                                    enabled = !busy && toolInput.isNotBlank(),
                                ) { Text("Freeze Tools") }
                            }
                        }
                    }

                    item {
                        Card(Modifier.fillMaxWidth()) {
                            Column(Modifier.padding(Spacing.md)) {
                                EstopRow(
                                    icon = Icons.Default.Shield,
                                    title = "Kill All",
                                    subtitle = "Aborts all agent turns before they reach the model.",
                                    active = current?.killAll == true,
                                    onResume = { handleResume(ResumeArgs()) },
                                    busy = busy,
                                )
                                Spacer(Modifier.height(Spacing.sm))
                                EstopRow(
                                    icon = Icons.Default.Public,
                                    title = "Network Kill",
                                    subtitle = "Blocks all outbound network-capable tool calls.",
                                    active = current?.networkKill == true,
                                    onResume = { handleResume(ResumeArgs(network = true)) },
                                    busy = busy,
                                )
                                Spacer(Modifier.height(Spacing.sm))
                                EstopListRow(
                                    icon = Icons.Default.Block,
                                    title = "Blocked Domains",
                                    subtitle = "Outbound requests to these hosts are refused.",
                                    items = current?.blockedDomains.orEmpty(),
                                    onResumeAll = { items -> handleResume(ResumeArgs(domains = items)) },
                                    busy = busy,
                                )
                                Spacer(Modifier.height(Spacing.sm))
                                EstopListRow(
                                    icon = Icons.Default.Build,
                                    title = "Frozen Tools",
                                    subtitle = "These tools refuse to execute until resumed.",
                                    items = current?.frozenTools.orEmpty(),
                                    onResumeAll = { items -> handleResume(ResumeArgs(tools = items)) },
                                    busy = busy,
                                )
                            }
                        }
                    }
                }
            }
        }
    }

    otpPrompt?.let { pending ->
        AlertDialog(
            onDismissRequest = { otpPrompt = null },
            icon = { Icon(Icons.Default.WarningAmber, contentDescription = null) },
            title = { Text("OTP Required") },
            text = {
                Column {
                    Text(
                        "Resuming from emergency stop requires a one-time passcode.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Spacer(Modifier.height(Spacing.md))
                    OutlinedTextField(
                        value = otpCode,
                        onValueChange = { otpCode = it },
                        label = { Text("OTP code") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            },
            confirmButton = {
                TextButton(
                    onClick = { resume(pending, otpCode.trim()) },
                    enabled = !busy && otpCode.isNotBlank(),
                ) { Text("Confirm Resume") }
            },
            dismissButton = {
                TextButton(onClick = { otpPrompt = null }) { Text("Cancel") }
            },
        )
    }
}

@Composable
private fun EstopBadge(active: Boolean, activeLabel: String, inactiveLabel: String) {
    Surface(
        color = if (active) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.surfaceVariant,
        shape = MaterialTheme.shapes.small,
    ) {
        Text(
            if (active) activeLabel else inactiveLabel,
            modifier = Modifier.padding(horizontal = Spacing.sm, vertical = 2.dp),
            style = MaterialTheme.typography.labelSmall,
            color = if (active) {
                MaterialTheme.colorScheme.onErrorContainer
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
        )
    }
}

@Composable
private fun EstopRow(
    icon: ImageVector,
    title: String,
    subtitle: String,
    active: Boolean,
    busy: Boolean,
    onResume: () -> Unit,
) {
    Row(
        Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Spacing.sm),
    ) {
        Icon(icon, contentDescription = null, modifier = Modifier.size(20.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyMedium)
            Text(subtitle, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        EstopBadge(active = active, activeLabel = "Active", inactiveLabel = "Inactive")
        if (active) {
            TextButton(onClick = onResume, enabled = !busy) { Text("Resume") }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun EstopListRow(
    icon: ImageVector,
    title: String,
    subtitle: String,
    items: List<String>,
    busy: Boolean,
    onResumeAll: (List<String>) -> Unit,
) {
    Column {
        Row(
            Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Spacing.sm),
        ) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(20.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.bodyMedium)
                Text(subtitle, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (items.isNotEmpty()) {
                TextButton(onClick = { onResumeAll(items) }, enabled = !busy) { Text("Resume all") }
            }
        }
        if (items.isEmpty()) {
            Text(
                "(none)",
                modifier = Modifier.padding(top = Spacing.xs),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
            )
        } else {
            FlowRow(
                modifier = Modifier.padding(top = Spacing.xs),
                horizontalArrangement = Arrangement.spacedBy(Spacing.xs),
            ) {
                items.forEach { entry ->
                    Surface(
                        color = MaterialTheme.colorScheme.errorContainer,
                        shape = MaterialTheme.shapes.extraSmall,
                    ) {
                        Text(
                            entry,
                            modifier = Modifier.padding(horizontal = Spacing.sm, vertical = 2.dp),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onErrorContainer,
                        )
                    }
                }
            }
        }
    }
}
