<script lang="ts">
	import * as Card from "$lib/components/ui/card";
	import { Button } from "$lib/components/ui/button";
	import { Badge } from "$lib/components/ui/badge";
	import Separator from "$lib/components/ui/separator/separator.svelte";
	import * as Tabs from "$lib/components/ui/tabs";

	interface ProcessResult {
		success: boolean;
		scrubbed_data: string | null;
		scrubbed_preview: string | null;
		file_name: string;
		file_type: string;
		stats: {
			total_pii_found: number;
			pii_by_type: Record<string, number>;
			rows_affected: number;
			cells_affected: number;
		};
		error: string | null;
	}

	type Props = {
		results: ProcessResult[];
		onDownload: (result: ProcessResult) => void;
		onDownloadAll: () => void;
		onReset: () => void;
		onBack: () => void;
	};

	let { results, onDownload, onDownloadAll, onReset, onBack }: Props =
		$props();

	let successfulResults = $derived(results.filter((r) => r.success));
	let totalPiiFound = $derived(
		results.reduce((sum, r) => sum + (r.stats?.total_pii_found || 0), 0),
	);
	let totalRowsAffected = $derived(
		results.reduce((sum, r) => sum + (r.stats?.rows_affected || 0), 0),
	);

	function getPiiTypeColor(type: string): string {
		const colors: Record<string, string> = {
			"Email Address": "bg-blue-500/10 text-blue-600 border-blue-200",
			"Phone Number": "bg-green-500/10 text-green-600 border-green-200",
			"Social Security Number":
				"bg-red-500/10 text-red-600 border-red-200",
			"Credit Card Number":
				"bg-purple-500/10 text-purple-600 border-purple-200",
			"IP Address": "bg-orange-500/10 text-orange-600 border-orange-200",
			"Date of Birth": "bg-pink-500/10 text-pink-600 border-pink-200",
			"Street Address":
				"bg-yellow-500/10 text-yellow-700 border-yellow-200",
			"Person Name": "bg-teal-500/10 text-teal-600 border-teal-200",
		};
		return colors[type] || "bg-gray-500/10 text-gray-600 border-gray-200";
	}
</script>

<div class="space-y-6">
	<!-- Summary Card -->
	<Card.Root>
		<Card.Header>
			<div class="flex items-center justify-between">
				<div>
					<Card.Title class="flex items-center space-x-2">
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-6 w-6 text-green-500"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								fill-rule="evenodd"
								d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
								clip-rule="evenodd"
							/>
						</svg>
						<span>Scrubbing Complete</span>
					</Card.Title>
					<Card.Description class="mt-1">
						Processed {results.length} file(s) successfully
					</Card.Description>
				</div>
				<div class="flex space-x-2">
					<Button variant="outline" onclick={onBack}>
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-4 w-4 mr-2"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								fill-rule="evenodd"
								d="M9.707 16.707a1 1 0 01-1.414 0l-6-6a1 1 0 010-1.414l6-6a1 1 0 011.414 1.414L5.414 9H17a1 1 0 110 2H5.414l4.293 4.293a1 1 0 010 1.414z"
								clip-rule="evenodd"
							/>
						</svg>
						Back
					</Button>
					<Button variant="outline" onclick={onReset}>
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-4 w-4 mr-2"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								fill-rule="evenodd"
								d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z"
								clip-rule="evenodd"
							/>
						</svg>
						Start Over
					</Button>
				</div>
			</div>
		</Card.Header>
		<Card.Content>
			<div class="grid grid-cols-3 gap-4">
				<div class="text-center p-4 bg-muted rounded-lg">
					<p class="text-3xl font-bold text-primary">
						{totalPiiFound}
					</p>
					<p class="text-sm text-muted-foreground">PII Items Found</p>
				</div>
				<div class="text-center p-4 bg-muted rounded-lg">
					<p class="text-3xl font-bold text-primary">
						{totalRowsAffected}
					</p>
					<p class="text-sm text-muted-foreground">Rows Affected</p>
				</div>
				<div class="text-center p-4 bg-muted rounded-lg">
					<p class="text-3xl font-bold text-primary">
						{successfulResults.length}
					</p>
					<p class="text-sm text-muted-foreground">Files Processed</p>
				</div>
			</div>
		</Card.Content>
	</Card.Root>

	<!-- File Results -->
	{#if results.length > 1}
		<Tabs.Root value={results[0].file_name}>
			<Tabs.List class="w-full justify-start overflow-x-auto">
				{#each results as result, i}
					<Tabs.Trigger
						value={result.file_name}
						class="flex items-center space-x-2"
					>
						{#if result.success}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								class="h-4 w-4 text-green-500"
								viewBox="0 0 20 20"
								fill="currentColor"
							>
								<path
									fill-rule="evenodd"
									d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
									clip-rule="evenodd"
								/>
							</svg>
						{:else}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								class="h-4 w-4 text-destructive"
								viewBox="0 0 20 20"
								fill="currentColor"
							>
								<path
									fill-rule="evenodd"
									d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z"
									clip-rule="evenodd"
								/>
							</svg>
						{/if}
						<span class="truncate max-w-37.5"
							>{result.file_name}</span
						>
					</Tabs.Trigger>
				{/each}
			</Tabs.List>
			{#each results as result, i}
				<Tabs.Content value={result.file_name}>
					<Card.Root>
						<Card.Header>
							<div class="flex items-center justify-between">
								<div>
									<Card.Title class="text-lg"
										>{result.file_name}</Card.Title
									>
									<Card.Description>
										{#if result.success}
											Found {result.stats.total_pii_found}
											PII items in {result.stats
												.cells_affected} cells
										{:else}
											Processing failed
										{/if}
									</Card.Description>
								</div>
								{#if result.success}
									<Button onclick={() => onDownload(result)}>
										<svg
											xmlns="http://www.w3.org/2000/svg"
											class="h-4 w-4 mr-2"
											viewBox="0 0 20 20"
											fill="currentColor"
										>
											<path
												fill-rule="evenodd"
												d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z"
												clip-rule="evenodd"
											/>
										</svg>
										Download
									</Button>
								{/if}
							</div>
						</Card.Header>
						<Card.Content>
							{#if result.success}
								<!-- PII Breakdown -->
								{#if Object.keys(result.stats.pii_by_type).length > 0}
									<div class="mb-6">
										<h4 class="text-sm font-medium mb-3">
											PII Types Detected
										</h4>
										<div class="flex flex-wrap gap-2">
											{#each Object.entries(result.stats.pii_by_type) as [type, count]}
												<Badge
													variant="outline"
													class={getPiiTypeColor(
														type,
													)}
												>
													{type}: {count}
												</Badge>
											{/each}
										</div>
									</div>
								{:else}
									<div
										class="mb-6 p-4 bg-muted rounded-lg text-center"
									>
										<svg
											xmlns="http://www.w3.org/2000/svg"
											class="h-8 w-8 mx-auto text-muted-foreground mb-2"
											viewBox="0 0 20 20"
											fill="currentColor"
										>
											<path
												fill-rule="evenodd"
												d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
												clip-rule="evenodd"
											/>
										</svg>
										<p
											class="text-sm text-muted-foreground"
										>
											No PII detected in this file
										</p>
									</div>
								{/if}

								<Separator class="my-4" />

								<!-- Preview -->
								<div>
									<h4 class="text-sm font-medium mb-3">
										Output Preview
									</h4>
									<div
										class="bg-muted rounded-lg p-4 max-h-75 overflow-auto"
									>
										<pre
											class="text-xs font-mono whitespace-pre-wrap break-all">{result.scrubbed_preview ||
												"No preview available"}</pre>
									</div>
								</div>
							{:else}
								<div class="p-4 bg-destructive/10 rounded-lg">
									<p class="text-destructive font-medium">
										Error
									</p>
									<p class="text-sm text-destructive/80 mt-1">
										{result.error}
									</p>
								</div>
							{/if}
						</Card.Content>
					</Card.Root>
				</Tabs.Content>
			{/each}
		</Tabs.Root>
	{/if}

	<!-- Download All Button -->
	{#if successfulResults.length > 1}
		<div class="flex justify-center pt-4">
			<Button size="lg" onclick={onDownloadAll} class="px-8">
				<svg
					xmlns="http://www.w3.org/2000/svg"
					class="h-5 w-5 mr-2"
					viewBox="0 0 20 20"
					fill="currentColor"
				>
					<path
						d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zM6.293 9.293a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z"
					/>
				</svg>
				Download All ({successfulResults.length} files)
			</Button>
		</div>
	{/if}

	<!-- Privacy Note -->
	<Card.Root class="bg-primary/5 border-primary/20">
		<Card.Content class="pt-6">
			<div class="flex items-start space-x-3">
				<svg
					xmlns="http://www.w3.org/2000/svg"
					class="h-5 w-5 text-primary mt-0.5"
					viewBox="0 0 20 20"
					fill="currentColor"
				>
					<path
						fill-rule="evenodd"
						d="M2.166 4.999A11.954 11.954 0 0010 1.944 11.954 11.954 0 0017.834 5c.11.65.166 1.32.166 2.001 0 5.225-3.34 9.67-8 11.317C5.34 16.67 2 12.225 2 7c0-.682.057-1.35.166-2.001zm11.541 3.708a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
						clip-rule="evenodd"
					/>
				</svg>
				<div>
					<p class="text-sm font-medium text-primary">
						Your Privacy Matters
					</p>
					<p class="text-xs text-muted-foreground mt-1">
						All processing happens locally on your device. Your
						files are never uploaded to any server. The original
						files remain unchanged — only the downloaded scrubbed
						versions contain modifications.
					</p>
				</div>
			</div>
		</Card.Content>
	</Card.Root>
</div>
