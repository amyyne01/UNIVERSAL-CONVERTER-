import { render, screen } from '@testing-library/react';
import { SourceMonogram } from '@/components/ui/SourceMonogram';
import { SOURCE_META } from '@/constants';
import { SOURCE_PLATFORMS } from '@shared/types';

describe('SourceMonogram', () => {
  it('has a SOURCE_META entry for every SourcePlatform', () => {
    expect(Object.keys(SOURCE_META).sort()).toEqual([...SOURCE_PLATFORMS].sort());
  });

  it.each(SOURCE_PLATFORMS)('%s renders its letters and is named by its label', (source) => {
    render(<SourceMonogram source={source} />);
    const tile = screen.getByRole('img', { name: SOURCE_META[source].label });
    expect(tile).toHaveTextContent(SOURCE_META[source].mono);
  });
});
