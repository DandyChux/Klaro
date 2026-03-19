<script lang="ts">
	import { Checkbox } from "$lib/components/ui/checkbox";
	import { Label } from "$lib/components/ui/label";
	import { Button } from "$lib/components/ui/button";
	import { Badge } from "./ui/badge";

	interface PiiTypeInfo {
		id: string;
		name: string;
		description: string;
		available_in_lite: boolean;
	}

	type Props = {
		piiTypes: PiiTypeInfo[];
		selected: string[];
		onChange: (types: string[]) => void;
	};

	let { piiTypes, selected, onChange }: Props = $props();

	let availableTypes = $derived(piiTypes.filter((t) => t.available_in_lite));
	let selectedAvailableCount = $derived(
		selected.filter((id) => availableTypes.some((t) => t.id === id)).length,
	);

	function toggleType(id: string) {
		// Find the type and check if it's available
		const piiType = piiTypes.find((t) => t.id === id);
		if (!piiType?.available_in_lite) return;

		const newSelected = selected.includes(id)
			? selected.filter((t) => t !== id)
			: [...selected, id];
		onChange(newSelected);
	}

	function selectAll() {
		const allAvailableIds = availableTypes.map((t) => t.id);
		onChange(allAvailableIds);
	}

	function selectNone() {
		onChange([]);
	}

	let allSelected = $derived(
		selectedAvailableCount === availableTypes.length,
	);
	let noneSelected = $derived(selected.length === 0);
</script>

<div class="space-y-4">
	<div class="flex items-center justify-between">
		<p class="text-sm text-muted-foreground">
			{selectedAvailableCount} of {availableTypes.length} selected
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

	<div class="grid gap-3 max-h-100 overflow-y-auto pr-2">
		{#each piiTypes as piiType}
			{@const isAvailable = piiType.available_in_lite}
			{@const isSelected = selected.includes(piiType.id)}

			<button
				class="flex items-start space-x-3 p-3 rounded-lg border transition-all text-left {isAvailable
					? isSelected
						? 'border-primary bg-primary/5 hover:bg-primary/10'
						: 'border-border hover:border-primary/30 hover:bg-muted/50'
					: 'border-border bg-muted/30 opacity-60 cursor-not-allowed'}"
				onclick={() => toggleType(piiType.id)}
				disabled={!isAvailable}
			>
				<Checkbox
					checked={selected.includes(piiType.id)}
					disabled={!isAvailable}
					class="mt-0.5"
				/>
				<div class="flex-1 min-w-0">
					<Label class="text-sm font-medium cursor-pointer"
						>{piiType.name}</Label
					>
					{#if !isAvailable}
						<Badge
							variant="outline"
							class="text-[10px] px-1.5 py-0 h-5 bg-amber-500/10 text-amber-600 border-amber-300"
						>
							PRO
						</Badge>
					{/if}
					<p class="text-xs text-muted-foreground mt-0.5 truncate">
						{piiType.description}
					</p>
					{#if !isAvailable}
						<p class="text-[10px] text-amber-600 mt-1">
							Upgrade to Pro to unlock
						</p>
					{/if}
				</div>
			</button>
		{/each}
	</div>

	<!-- Pro upgrade hint if any types are locked -->
	{#if piiTypes.some((t) => !t.available_in_lite)}
		<div
			class="flex items-center gap-2 p-3 rounded-lg bg-amber-500/10 border border-amber-300/50"
		>
			<svg
				xmlns="http://www.w3.org/2000/svg"
				class="h-4 w-4 text-amber-600 shrink-0"
				viewBox="0 0 20 20"
				fill="currentColor"
			>
				<path
					fill-rule="evenodd"
					d="M5 2a2 2 0 00-2 2v14l3.5-2 3.5 2 3.5-2 3.5 2V4a2 2 0 00-2-2H5zm2.5 3a1.5 1.5 0 100 3 1.5 1.5 0 000-3zm6.207.293a1 1 0 00-1.414 0l-6 6a1 1 0 101.414 1.414l6-6a1 1 0 000-1.414zM12.5 10a1.5 1.5 0 100 3 1.5 1.5 0 000-3z"
					clip-rule="evenodd"
				/>
			</svg>
			<p class="text-xs text-amber-700">
				<span class="font-medium">Upgrade to Pro</span> to unlock all {piiTypes.length -
					availableTypes.length} additional PII types
			</p>
		</div>
	{/if}
</div>
