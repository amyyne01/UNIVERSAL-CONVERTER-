import { Loading } from '@/components/ui/icons';

export interface SpinnerProps {
  size?: number;
  className?: string;
}

export function Spinner({ size = 16, className = '' }: SpinnerProps) {
  return <Loading size={size} weight="thin" className={`animate-spin ${className}`} />;
}
