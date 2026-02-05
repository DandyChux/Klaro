<script lang="ts">
	import { Label } from "$lib/components/ui/label";

	type Props = {
		selected: string;
		onChange: (method: string) => void;
	};

	let { selected = "mask", onChange }: Props = $props();

	interface ScrubMethod {
		id: string;
		name: string;
		description: string;
		example: { before: string; after: string };
		icon: string;
	}

	const methods: ScrubMethod[] = [
		{
			id: "mask",
			name: "Mask",
			description: "Partially hide the data while preserving format",
			example: { before: "john@email.com", after: "j***@email.com" },
			icon: "mask",
		},
		{
			id: "remove",
			name: "Remove",
			description: "Replace with a placeholder label",
			example: { before: "john@email.com", after: "[EMAIL REMOVED]" },
			icon: "remove",
		},
		{
			id: "hash",
			name: "Hash",
			description: "One-way cryptographic hash (irreversible)",
			example: { before: "john@email.com", after: "#a1b2c3d4e5f6" },
			icon: "hash",
		},
		{
			id: "fake",
			name: "Fake",
			description: "Replace with realistic fake data",
			example: { before: "john@email.com", after: "sarah@example.net" },
			icon: "fake",
		},
	];

	function selectMethod(id: string) {
		selected = id;
		onChange(id);
	}
</script>

<div class="space-y-3">
	{#each methods as method}
		<button
			class="w-full p-4 rounded-lg border-2 text-left transition-all {selected ===
			method.id
				? 'border-primary bg-primary/5'
				: 'border-border hover:border-primary/50 hover:bg-muted/50'}"
			onclick={() => selectMethod(method.id)}
		>
			<div class="flex items-start space-x-3">
				<div
					class="w-10 h-10 rounded-lg flex items-center justify-center {selected ===
					method.id
						? 'bg-primary text-primary-foreground'
						: 'bg-muted text-muted-foreground'}"
				>
					{#if method.icon === "mask"}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-5 w-5"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								fill-rule="evenodd"
								d="M3.707 2.293a1 1 0 00-1.414 1.414l14 14a1 1 0 001.414-1.414l-1.473-1.473A10.014 10.014 0 0019.542 10C18.268 5.943 14.478 3 10 3a9.958 9.958 0 00-4.512 1.074l-1.78-1.781zm4.261 4.26l1.514 1.515a2.003 2.003 0 012.45 2.45l1.514 1.514a4 4 0 00-5.478-5.478z"
								clip-rule="evenodd"
							/>
							<path
								d="M12.454 16.697L9.75 13.992a4 4 0 01-3.742-3.741L2.335 6.578A9.98 9.98 0 00.458 10c1.274 4.057 5.065 7 9.542 7 .847 0 1.669-.105 2.454-.303z"
							/>
						</svg>
					{:else if method.icon === "remove"}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-5 w-5"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								fill-rule="evenodd"
								d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z"
								clip-rule="evenodd"
							/>
						</svg>
					{:else if method.icon === "hash"}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-5 w-5"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								fill-rule="evenodd"
								d="M9.243 3.03a1 1 0 01.727 1.213L9.53 6h2.94l.56-2.243a1 1 0 111.94.486L14.53 6H17a1 1 0 110 2h-2.97l-1 4H15a1 1 0 110 2h-2.47l-.56 2.242a1 1 0 11-1.94-.485L10.47 14H7.53l-.56 2.242a1 1 0 11-1.94-.485L5.47 14H3a1 1 0 110-2h2.97l1-4H5a1 1 0 110-2h2.47l.56-2.243a1 1 0 011.213-.727zM9.03 8l-1 4h2.938l1-4H9.031z"
								clip-rule="evenodd"
							/>
						</svg>
					{:else if method.icon === "fake"}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-5 w-5"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								d="M10 2a5 5 0 00-5 5v2a2 2 0 00-2 2v5a2 2 0 002 2h10a2 2 0 002-2v-5a2 2 0 00-2-2H7V7a3 3 0 015.905-.75 1 1 0 001.937-.5A5.002 5.002 0 0010 2z"
							/>
						</svg>
					{/if}
				</div>
				<div class="flex-1">
					<div class="flex items-center justify-between">
						<Label class="text-base font-semibold cursor-pointer"
							>{method.name}</Label
						>
						{#if selected === method.id}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								class="h-5 w-5 text-primary"
								viewBox="0 0 20 20"
								fill="currentColor"
							>
								<path
									fill-rule="evenodd"
									d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
									clip-rule="evenodd"
								/>
							</svg>
						{/if}
					</div>
					<p class="text-sm text-muted-foreground mt-1">
						{method.description}
					</p>
					<div class="mt-3 p-2 bg-muted rounded text-xs font-mono">
						<span class="text-destructive line-through"
							>{method.example.before}</span
						>
						<span class="mx-2">→</span>
						<span class="text-primary">{method.example.after}</span>
					</div>
				</div>
			</div>
		</button>
	{/each}
</div>
