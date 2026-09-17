<script lang="ts">
  import { clampLevel } from '../queue';

  export let level: number;
  export let caption = 'queue depth';
</script>

<meter value={clampLevel(level)} max="10" />
