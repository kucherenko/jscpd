<script lang="ts">
  import { clampLevel } from '../queue';
  import { gaugeTheme } from '../stores';

  export let level: number;
  export let caption = 'queue depth';
  const unit = 'jobs waiting';
</script>

<meter class={$gaugeTheme} value={clampLevel(level)} max="10" title={`${level} ${unit}`} />
