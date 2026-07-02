package com.agentzero.client.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.Dialpad
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.agentzero.client.data.model.ServerConfig
import com.agentzero.client.ui.components.ErrorBanner
import com.agentzero.client.ui.theme.Spacing

@Composable
private fun BrandMark() {
    Box(
        modifier = Modifier
            .size(64.dp)
            .background(MaterialTheme.colorScheme.primaryContainer, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            Icons.Default.Bolt,
            contentDescription = null,
            modifier = Modifier.size(32.dp),
            tint = MaterialTheme.colorScheme.onPrimaryContainer,
        )
    }
}

@Composable
fun ServerSetupScreen(
    onSave: (host: String, port: Int, onDone: (String?) -> Unit) -> Unit,
) {
    var host by remember { mutableStateOf("") }
    var portText by remember { mutableStateOf("42617") }
    var error by remember { mutableStateOf<String?>(null) }
    var loading by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(Spacing.xl),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        BrandMark()
        Spacer(modifier = Modifier.height(Spacing.lg))
        Text(
            text = "AgentZero",
            style = MaterialTheme.typography.headlineMedium,
        )
        Spacer(modifier = Modifier.height(Spacing.xs))
        Text(
            text = "Connect to your gateway over Tailscale",
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(Spacing.xxl))

        OutlinedTextField(
            value = host,
            onValueChange = { host = it },
            label = { Text("Host") },
            placeholder = { Text("100.x.x.x or machine-name") },
            singleLine = true,
            shape = MaterialTheme.shapes.medium,
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(modifier = Modifier.height(Spacing.md))
        OutlinedTextField(
            value = portText,
            onValueChange = { portText = it.filter { ch -> ch.isDigit() } },
            label = { Text("Port") },
            singleLine = true,
            shape = MaterialTheme.shapes.medium,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.fillMaxWidth(),
        )

        error?.let {
            Spacer(modifier = Modifier.height(Spacing.md))
            ErrorBanner(it)
        }

        Spacer(modifier = Modifier.height(Spacing.xl))
        Button(
            onClick = {
                loading = true
                error = null
                val port = portText.toIntOrNull() ?: 0
                onSave(host, port) { message ->
                    loading = false
                    error = message
                }
            },
            enabled = !loading,
            shape = MaterialTheme.shapes.medium,
            modifier = Modifier
                .fillMaxWidth()
                .height(48.dp),
        ) {
            if (loading) {
                CircularProgressIndicator(modifier = Modifier.size(20.dp))
            } else {
                Text("Connect")
            }
        }
    }
}

@Composable
fun PairingScreen(
    config: ServerConfig,
    onPair: (code: String, config: ServerConfig, onDone: (String?) -> Unit) -> Unit,
) {
    var code by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    var loading by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(Spacing.xl),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        BrandMark()
        Spacer(modifier = Modifier.height(Spacing.lg))
        Text(
            text = "AgentZero",
            style = MaterialTheme.typography.headlineMedium,
        )
        Spacer(modifier = Modifier.height(Spacing.xs))
        Text(
            text = "Enter the one-time pairing code from your terminal",
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(Spacing.sm))
        Text(
            text = config.baseUrl,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.primary,
        )
        Spacer(modifier = Modifier.height(Spacing.xxl))

        OutlinedTextField(
            value = code,
            onValueChange = { if (it.length <= 6) code = it.filter { ch -> ch.isDigit() } },
            label = { Text("Pairing code") },
            placeholder = { Text("6-digit code") },
            singleLine = true,
            shape = MaterialTheme.shapes.medium,
            leadingIcon = { Icon(Icons.Default.Dialpad, contentDescription = null) },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.fillMaxWidth(),
        )

        error?.let {
            Spacer(modifier = Modifier.height(Spacing.md))
            ErrorBanner(it)
        }

        Spacer(modifier = Modifier.height(Spacing.xl))
        Button(
            onClick = {
                loading = true
                error = null
                onPair(code, config) { message ->
                    loading = false
                    error = message
                }
            },
            enabled = !loading && code.length >= 6,
            shape = MaterialTheme.shapes.medium,
            modifier = Modifier
                .fillMaxWidth()
                .height(48.dp),
        ) {
            if (loading) {
                CircularProgressIndicator(modifier = Modifier.size(20.dp))
            } else {
                Text("Pair")
            }
        }
    }
}
