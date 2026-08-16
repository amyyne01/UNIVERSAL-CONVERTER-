// The renderer has no other net. React unmounts the WHOLE tree when a render
// throws, so one bad field on one row used to leave the user staring at an empty
// window with no rail, no title bar and no way back — the app looked dead when
// only a tab was. This confines that to the tab: everything outside it keeps
// working, and switching tabs (the key changes) mounts a clean one.
import { Component, type ReactNode } from 'react';
import { Alert, Retry } from '@/components/ui/icons';
import { Button } from '@/components/ui';

interface Props { children: ReactNode }
interface State { error: Error | null }

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="mx-auto w-full max-w-[720px] px-6 py-16">
        <div
          role="alert"
          className="rounded-lg border p-6"
          style={{
            borderColor: 'color-mix(in oklch, var(--color-error) 38%, transparent)',
            background: 'color-mix(in oklch, var(--color-error) 9%, transparent)',
          }}
        >
          <Alert size={22} className="text-error" />
          <h2 className="text-h2 text-text-primary mt-3">This tab couldn’t be drawn</h2>
          <p className="mt-2 text-sm text-text-secondary leading-relaxed">
            The rest of the app is unaffected — your downloads are still running. Try again, or
            switch to another tab.
          </p>
          <p className="mt-3 font-mono text-[11px] text-text-muted break-all">{error.message}</p>
          <Button
            className="mt-5"
            icon={Retry}
            onClick={() => this.setState({ error: null })}
          >
            Try again
          </Button>
        </div>
      </div>
    );
  }
}
