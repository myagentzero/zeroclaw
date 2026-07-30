import { useState, useEffect, useCallback } from 'react';
import { OctagonAlert, ShieldCheck, Globe, Ban, Wrench, KeyRound } from 'lucide-react';
import type { EstopStatus } from '@/types/api';
import { getEstopStatus, engageEstop, resumeEstop, type EstopEngageLevel } from '@/lib/api';
import { useSSE } from '@/hooks/useSSE';

type ResumeArgs = Parameters<typeof resumeEstop>[0];

function Badge({ active, activeLabel, inactiveLabel }: { active: boolean; activeLabel: string; inactiveLabel: string }) {
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${
        active
          ? 'bg-red-900/40 text-red-400 border border-red-700/50'
          : 'bg-gray-800 text-gray-500 border border-gray-700'
      }`}
    >
      {active ? activeLabel : inactiveLabel}
    </span>
  );
}

export default function Estop() {
  const [status, setStatus] = useState<EstopStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [disabled, setDisabled] = useState(false);

  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  const [domainInput, setDomainInput] = useState('');
  const [toolInput, setToolInput] = useState('');
  const [otpPrompt, setOtpPrompt] = useState<ResumeArgs | null>(null);
  const [otpCode, setOtpCode] = useState('');

  const { events } = useSSE({ filterTypes: ['estop_status'] });

  const refresh = useCallback(async () => {
    try {
      const data = await getEstopStatus();
      setStatus(data);
      setDisabled(false);
      setLoadError(null);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Failed to load estop status';
      if (message.toLowerCase().includes('disabled')) {
        setDisabled(true);
      } else {
        setLoadError(message);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const last = events[events.length - 1];
    if (!last) return;
    setStatus((prev) => ({
      enabled: true,
      is_engaged: Boolean(last.is_engaged),
      kill_all: Boolean(last.kill_all),
      network_kill: Boolean(last.network_kill),
      blocked_domains: Array.isArray(last.blocked_domains)
        ? (last.blocked_domains as string[])
        : prev?.blocked_domains ?? [],
      frozen_tools: Array.isArray(last.frozen_tools)
        ? (last.frozen_tools as string[])
        : prev?.frozen_tools ?? [],
      updated_at: (last.updated_at as string | undefined) ?? prev?.updated_at ?? null,
      require_otp_to_resume: prev?.require_otp_to_resume ?? false,
    }));
  }, [events]);

  const doEngage = async (level: EstopEngageLevel, opts: { domains?: string[]; tools?: string[] } = {}) => {
    setBusy(true);
    setActionError(null);
    setActionMessage(null);
    try {
      const data = await engageEstop(level, opts);
      setStatus(data);
      setActionMessage('Emergency stop engaged.');
    } catch (err: unknown) {
      setActionError(err instanceof Error ? err.message : 'Failed to engage emergency stop');
    } finally {
      setBusy(false);
    }
  };

  const doResume = async (args: ResumeArgs) => {
    setBusy(true);
    setActionError(null);
    setActionMessage(null);
    try {
      const data = await resumeEstop(args);
      setStatus(data);
      setActionMessage('Resume completed.');
      setOtpPrompt(null);
      setOtpCode('');
    } catch (err: unknown) {
      setActionError(err instanceof Error ? err.message : 'Failed to resume');
    } finally {
      setBusy(false);
    }
  };

  const handleResume = (args: ResumeArgs) => {
    if (status?.require_otp_to_resume) {
      setOtpPrompt(args);
      setOtpCode('');
      return;
    }
    doResume(args);
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-2 border-red-500 border-t-transparent" />
      </div>
    );
  }

  if (disabled) {
    return (
      <div className="p-6 space-y-6">
        <div className="flex items-center gap-2">
          <OctagonAlert className="h-5 w-5 text-red-400" />
          <h2 className="text-base font-semibold text-white">Emergency Stop</h2>
        </div>
        <div className="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center">
          <ShieldCheck className="h-10 w-10 text-gray-600 mx-auto mb-3" />
          <p className="text-gray-300 font-medium">Emergency stop is disabled</p>
          <p className="text-sm text-gray-500 mt-1">
            Set <code className="text-gray-400">[security.estop] enabled = true</code> in{' '}
            <code className="text-gray-400">config.toml</code> and restart AgentZero to enable
            engage/resume controls here.
          </p>
        </div>
      </div>
    );
  }

  if (loadError && !status) {
    return (
      <div className="p-6">
        <div className="rounded-lg bg-red-900/30 border border-red-700 p-4 text-red-300">
          Failed to load emergency stop status: {loadError}
        </div>
      </div>
    );
  }

  const engaged = status?.is_engaged ?? false;

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <OctagonAlert className={`h-5 w-5 ${engaged ? 'text-red-400' : 'text-gray-500'}`} />
          <h2 className="text-base font-semibold text-white">Emergency Stop</h2>
        </div>
        <Badge active={engaged} activeLabel="Engaged" inactiveLabel="Clear" />
      </div>

      {actionError && (
        <div className="rounded-lg bg-red-900/30 border border-red-700 p-3 text-sm text-red-300">
          {actionError}
        </div>
      )}
      {actionMessage && (
        <div className="rounded-lg bg-green-900/20 border border-green-700/50 p-3 text-sm text-green-300">
          {actionMessage}
        </div>
      )}

      {/* Status */}
      <div className="bg-gray-900 rounded-xl border border-gray-800 divide-y divide-gray-800">
        <div className="flex items-center justify-between p-4">
          <div className="flex items-center gap-3">
            <ShieldCheck className="h-4 w-4 text-gray-400" />
            <div>
              <p className="text-sm text-white">Kill All</p>
              <p className="text-xs text-gray-500">Aborts all agent turns before they reach the model.</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <Badge active={status?.kill_all ?? false} activeLabel="Active" inactiveLabel="Inactive" />
            {status?.kill_all && (
              <button
                disabled={busy}
                onClick={() => handleResume({})}
                className="text-xs font-medium text-blue-400 hover:text-blue-300 disabled:opacity-50"
              >
                Resume
              </button>
            )}
          </div>
        </div>

        <div className="flex items-center justify-between p-4">
          <div className="flex items-center gap-3">
            <Globe className="h-4 w-4 text-gray-400" />
            <div>
              <p className="text-sm text-white">Network Kill</p>
              <p className="text-xs text-gray-500">Blocks all outbound network-capable tool calls.</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <Badge active={status?.network_kill ?? false} activeLabel="Active" inactiveLabel="Inactive" />
            {status?.network_kill && (
              <button
                disabled={busy}
                onClick={() => handleResume({ network: true })}
                className="text-xs font-medium text-blue-400 hover:text-blue-300 disabled:opacity-50"
              >
                Resume
              </button>
            )}
          </div>
        </div>

        <div className="p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Ban className="h-4 w-4 text-gray-400" />
              <div>
                <p className="text-sm text-white">Blocked Domains</p>
                <p className="text-xs text-gray-500">Outbound requests to these hosts are refused.</p>
              </div>
            </div>
            {(status?.blocked_domains?.length ?? 0) > 0 && (
              <button
                disabled={busy}
                onClick={() => handleResume({ domains: status?.blocked_domains ?? [] })}
                className="text-xs font-medium text-blue-400 hover:text-blue-300 disabled:opacity-50"
              >
                Resume all
              </button>
            )}
          </div>
          {(status?.blocked_domains?.length ?? 0) > 0 ? (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {status?.blocked_domains.map((domain) => (
                <span
                  key={domain}
                  className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-mono bg-red-900/30 text-red-300 border border-red-700/40"
                >
                  {domain}
                </span>
              ))}
            </div>
          ) : (
            <p className="mt-1 text-xs text-gray-600">(none)</p>
          )}
        </div>

        <div className="p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Wrench className="h-4 w-4 text-gray-400" />
              <div>
                <p className="text-sm text-white">Frozen Tools</p>
                <p className="text-xs text-gray-500">These tools refuse to execute until resumed.</p>
              </div>
            </div>
            {(status?.frozen_tools?.length ?? 0) > 0 && (
              <button
                disabled={busy}
                onClick={() => handleResume({ tools: status?.frozen_tools ?? [] })}
                className="text-xs font-medium text-blue-400 hover:text-blue-300 disabled:opacity-50"
              >
                Resume all
              </button>
            )}
          </div>
          {(status?.frozen_tools?.length ?? 0) > 0 ? (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {status?.frozen_tools.map((tool) => (
                <span
                  key={tool}
                  className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-mono bg-red-900/30 text-red-300 border border-red-700/40"
                >
                  {tool}
                </span>
              ))}
            </div>
          ) : (
            <p className="mt-1 text-xs text-gray-600">(none)</p>
          )}
        </div>
      </div>

      {/* Engage controls */}
      <div className="bg-gray-900 rounded-xl border border-gray-800 p-4 space-y-4">
        <h3 className="text-sm font-semibold text-gray-400 uppercase tracking-wider">Engage</h3>
        <div className="flex flex-wrap gap-3">
          <button
            disabled={busy}
            onClick={() => doEngage('kill-all')}
            className="px-4 py-2 text-sm font-medium text-white bg-red-700 hover:bg-red-600 rounded-lg transition-colors disabled:opacity-50"
          >
            Kill All
          </button>
          <button
            disabled={busy}
            onClick={() => doEngage('network-kill')}
            className="px-4 py-2 text-sm font-medium text-white bg-orange-700 hover:bg-orange-600 rounded-lg transition-colors disabled:opacity-50"
          >
            Network Kill
          </button>
        </div>

        <div className="flex flex-wrap items-end gap-3">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Block domain(s)</label>
            <input
              type="text"
              value={domainInput}
              onChange={(e) => setDomainInput(e.target.value)}
              placeholder="e.g. *.example.com, other.com"
              className="w-64 bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-red-500"
            />
          </div>
          <button
            disabled={busy || !domainInput.trim()}
            onClick={() => {
              const domains = domainInput.split(',').map((d) => d.trim()).filter(Boolean);
              doEngage('domain-block', { domains });
              setDomainInput('');
            }}
            className="px-4 py-2 text-sm font-medium text-white bg-gray-700 hover:bg-gray-600 rounded-lg transition-colors disabled:opacity-50"
          >
            Block Domains
          </button>
        </div>

        <div className="flex flex-wrap items-end gap-3">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Freeze tool(s)</label>
            <input
              type="text"
              value={toolInput}
              onChange={(e) => setToolInput(e.target.value)}
              placeholder="e.g. shell, file_write"
              className="w-64 bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-red-500"
            />
          </div>
          <button
            disabled={busy || !toolInput.trim()}
            onClick={() => {
              const tools = toolInput.split(',').map((t) => t.trim()).filter(Boolean);
              doEngage('tool-freeze', { tools });
              setToolInput('');
            }}
            className="px-4 py-2 text-sm font-medium text-white bg-gray-700 hover:bg-gray-600 rounded-lg transition-colors disabled:opacity-50"
          >
            Freeze Tools
          </button>
        </div>
      </div>

      {/* OTP prompt for resume */}
      {otpPrompt && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
          <div className="bg-gray-900 border border-gray-700 rounded-xl p-6 w-full max-w-sm">
            <div className="flex items-center gap-2 mb-4">
              <KeyRound className="h-5 w-5 text-blue-400" />
              <h3 className="text-lg font-semibold text-white">OTP Required</h3>
            </div>
            <p className="text-sm text-gray-400 mb-4">
              Resuming from emergency stop requires a one-time passcode.
            </p>
            <input
              type="text"
              value={otpCode}
              onChange={(e) => setOtpCode(e.target.value)}
              placeholder="6-digit code"
              autoFocus
              maxLength={10}
              className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-center text-xl tracking-[0.3em] text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 mb-4"
            />
            <div className="flex justify-end gap-3">
              <button
                onClick={() => {
                  setOtpPrompt(null);
                  setOtpCode('');
                }}
                className="px-4 py-2 text-sm font-medium text-gray-300 hover:text-white border border-gray-700 rounded-lg hover:bg-gray-800 transition-colors"
              >
                Cancel
              </button>
              <button
                disabled={busy || otpCode.trim().length === 0}
                onClick={() => doResume({ ...otpPrompt, otpCode: otpCode.trim() })}
                className="px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors disabled:opacity-50"
              >
                Confirm Resume
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
