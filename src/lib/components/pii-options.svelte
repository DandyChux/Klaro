<script lang="ts">
	import { Checkbox } from "$lib/components/ui/checkbox";
	import { Label } from "$lib/components/ui/label";
	import { Button } from "$lib/components/ui/button";

	interface PiiTypeInfo {
		id: string;
		name: string;
		description: string;
	}

	export let piiTypes: PiiTypeInfo[] = [];
	export let selected: string[] = [];
	export let onChange: (types: string[]) => void;

	function toggleType(id: string) {
		if (selected.includes(id)) {
			selected = selected.filter((t) => t !== id);
		} else {
			selected = [...selected, id];
		}
		onChange(selected);
	}

	function selectAll() {
		selected = piiTypes.map((t) => t.id);
		onChange(selected);
	}

	function selectNone() {
		selected = [];
		onChange(selected);
	}

	$: allSelected = selected.length === piiTypes.length;
	$: noneSelected = selected.length === 0;
</script>

<div class="space-y-4">
	<div class="flex items-center justify-between">
		<p class="text-sm text-muted-foreground">
			{selected.length} of {piiTypes.length} selected
		</p>
		<div class="flex space-x-2">
			<Button
				variant="ghost"
				size="sm"
				onclick={selectAll}
				disabled={allSelected}
			>
				Select All
			</Button>
			<Button
				variant="ghost"
				size="sm"
				onclick={selectNone}
				disabled={noneSelected}
			>
				Clear
			</Button>
		</div>
	</div>

	<div class="grid gap-3 max-h-[400px] overflow-y-auto pr-2">
		{#each piiTypes as piiType}
			<button
				class="flex items-start space-x-3 p-3 rounded-lg border transition-all text-left {selected.includes(
					piiType.id,
				)
					? 'border-primary bg-primary/5'
					: 'border-border hover:border-primary/30 hover:bg-muted/50'}"
				onclick={() => toggleType(piiType.id)}
			>
				<Checkbox
					checked={selected.includes(piiType.id)}
					class="mt-0.5"
				/>
				<div class="flex-1 min-w-0">
					<Label class="text-sm font-medium cursor-pointer"
						>{piiType.name}</Label
					>
					<p class="text-xs text-muted-foreground mt-0.5 truncate">
						{piiType.description}
					</p>
				</div>
			</button>
		{/each}
	</div>
</div>
