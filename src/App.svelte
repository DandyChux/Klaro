<script lang="ts">
	import { invoke } from "@tauri-apps/api/tauri";
	import { onMount } from "svelte";
	import FileUpload from "$lib/components/file-upload.svelte";
	import PiiOptions from "$lib/components/pii-options.svelte";
	import ResultsPanel from "$lib/components/results-panel.svelte";
	import ScrubMethodSelect from "$lib/components/scrub-method-select.svelte";
	import Header from "$lib/components/header.svelte";
	import { Button } from "$lib/components/ui/button";
	import * as Card from "$lib/components/ui/card";
	import { Separator } from "$lib/components/ui/separator";
	import * as Alert from "$lib/components/ui/alert";

	interface PiiTypeInfo {
		id: string;
		name: string;
		description: string;
	}

	interface SupportedFormat {
		extension: string;
		name: string;
		mime_type: string;
	}

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

	interface UploadedFile {
		name: string;
		size: number;
		type: string;
		data: string; // base64
	}

	let piiTypes: PiiTypeInfo[] = [];
	let supportedFormats: SupportedFormat[] = [];
	let selectedPiiTypes: string[] = [];
	let scrubMethod: string = "mask";
	let uploadedFiles: UploadedFile[] = [];
	let results: ProcessResult[] = [];
	let isProcessing = false;
	let error: string | null = null;
	let currentStep: "upload" | "configure" | "results" = "upload";

	onMount(async () => {
		try {
			piiTypes = await invoke<PiiTypeInfo[]>("get_pii_types");
			supportedFormats = await invoke<SupportedFormat[]>(
				"get_supported_formats",
			);
			// Select all PII types by default
			selectedPiiTypes = piiTypes.map((t) => t.id);
		} catch (e) {
			error = `Failed to initialize: ${e}`;
		}
	});

	function handleFilesSelected(files: UploadedFile[]) {
		uploadedFiles = files;
		error = null;
		if (files.length > 0) {
			currentStep = "configure";
		}
	}

	function handlePiiTypesChange(types: string[]) {
		selectedPiiTypes = types;
	}

	function handleScrubMethodChange(method: string) {
		scrubMethod = method;
	}

	async function processFiles() {
		if (uploadedFiles.length === 0) {
			error = "Please upload at least one file";
			return;
		}

		if (selectedPiiTypes.length === 0) {
			error = "Please select at least one PII type to detect";
			return;
		}

		isProcessing = true;
		error = null;
		results = [];

		try {
			for (const file of uploadedFiles) {
				const result = await invoke<ProcessResult>("process_file", {
					request: {
						file_data: file.data,
						file_name: file.name,
						pii_types: selectedPiiTypes,
						scrub_method: scrubMethod,
					},
				});
				results = [...results, result];
			}
			currentStep = "results";
		} catch (e) {
			error = `Processing failed: ${e}`;
		} finally {
			isProcessing = false;
		}
	}

	function reset() {
		uploadedFiles = [];
		results = [];
		error = null;
		currentStep = "upload";
	}

	function goBack() {
		if (currentStep === "results") {
			currentStep = "configure";
		} else if (currentStep === "configure") {
			currentStep = "upload";
		}
	}

	function downloadFile(result: ProcessResult) {
		if (!result.scrubbed_data) return;

		const binaryString = atob(result.scrubbed_data);
		const bytes = new Uint8Array(binaryString.length);
		for (let i = 0; i < binaryString.length; i++) {
			bytes[i] = binaryString.charCodeAt(i);
		}

		const blob = new Blob([bytes], { type: "text/csv" });
		const url = URL.createObjectURL(blob);
		console.log("File URL:", url);
		const a = document.createElement("a");
		a.href = url;
		a.download = result.file_name;
		document.body.appendChild(a);
		a.click();
		document.body.removeChild(a);
		URL.revokeObjectURL(url);
		console.log("File downloaded");
	}

	function downloadAllFiles() {
		results.forEach((result) => {
			if (result.success) {
				downloadFile(result);
			}
		});
	}

	$: totalPiiFound = results.reduce(
		(sum, r) => sum + (r.stats?.total_pii_found || 0),
		0,
	);
	$: successfulResults = results.filter((r) => r.success);
</script>

<div class="min-h-screen bg-background">
	<Header />

	<main class="container mx-auto px-4 py-8 max-w-5xl">
		{#if error}
			<Alert.Root variant="destructive" class="mb-6">
				<svg
					xmlns="http://www.w3.org/2000/svg"
					class="h-4 w-4"
					viewBox="0 0 20 20"
					fill="currentColor"
				>
					<path
						fill-rule="evenodd"
						d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z"
						clip-rule="evenodd"
					/>
				</svg>
				<Alert.Title>Error</Alert.Title>
				<Alert.Description>{error}</Alert.Description>
			</Alert.Root>
		{/if}

		<!-- Progress Steps -->
		<div class="flex items-center justify-center mb-8">
			<div class="flex items-center space-x-4">
				<button
					class="flex items-center space-x-2 {currentStep === 'upload'
						? 'text-primary'
						: 'text-muted-foreground'}"
					onclick={() =>
						currentStep === "configure" && (currentStep = "upload")}
					disabled={currentStep === "results"}
				>
					<div
						class="w-8 h-8 rounded-full flex items-center justify-center {currentStep ===
						'upload'
							? 'bg-primary text-primary-foreground'
							: uploadedFiles.length > 0
								? 'bg-primary/20 text-primary'
								: 'bg-muted text-muted-foreground'}"
					>
						{#if uploadedFiles.length > 0 && currentStep !== "upload"}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								class="h-4 w-4"
								viewBox="0 0 20 20"
								fill="currentColor"
							>
								<path
									fill-rule="evenodd"
									d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
									clip-rule="evenodd"
								/>
							</svg>
						{:else}
							1
						{/if}
					</div>
					<span class="font-medium">Upload</span>
				</button>

				<div
					class="w-16 h-0.5 {currentStep !== 'upload'
						? 'bg-primary'
						: 'bg-muted'}"
				></div>

				<button
					class="flex items-center space-x-2 {currentStep ===
					'configure'
						? 'text-primary'
						: 'text-muted-foreground'}"
					disabled={uploadedFiles.length === 0}
				>
					<div
						class="w-8 h-8 rounded-full flex items-center justify-center {currentStep ===
						'configure'
							? 'bg-primary text-primary-foreground'
							: currentStep === 'results'
								? 'bg-primary/20 text-primary'
								: 'bg-muted text-muted-foreground'}"
					>
						{#if currentStep === "results"}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								class="h-4 w-4"
								viewBox="0 0 20 20"
								fill="currentColor"
							>
								<path
									fill-rule="evenodd"
									d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
									clip-rule="evenodd"
								/>
							</svg>
						{:else}
							2
						{/if}
					</div>
					<span class="font-medium">Configure</span>
				</button>

				<div
					class="w-16 h-0.5 {currentStep === 'results'
						? 'bg-primary'
						: 'bg-muted'}"
				></div>

				<div
					class="flex items-center space-x-2 {currentStep ===
					'results'
						? 'text-primary'
						: 'text-muted-foreground'}"
				>
					<div
						class="w-8 h-8 rounded-full flex items-center justify-center {currentStep ===
						'results'
							? 'bg-primary text-primary-foreground'
							: 'bg-muted text-muted-foreground'}"
					>
						3
					</div>
					<span class="font-medium">Results</span>
				</div>
			</div>
		</div>

		<!-- Step Content -->
		{#if currentStep === "upload"}
			<FileUpload
				{supportedFormats}
				onFilesSelected={handleFilesSelected}
			/>
		{:else if currentStep === "configure"}
			<div class="grid gap-6 md:grid-cols-2">
				<Card.Root>
					<Card.Header>
						<Card.Title>PII Types to Detect</Card.Title>
						<Card.Description>
							Select which types of personal information to find
							and scrub
						</Card.Description>
					</Card.Header>
					<Card.Content>
						<PiiOptions
							{piiTypes}
							selected={selectedPiiTypes}
							onChange={handlePiiTypesChange}
						/>
					</Card.Content>
				</Card.Root>

				<Card.Root>
					<Card.Header>
						<Card.Title>Scrubbing Method</Card.Title>
						<Card.Description>
							Choose how to handle detected PII
						</Card.Description>
					</Card.Header>
					<Card.Content>
						<ScrubMethodSelect
							selected={scrubMethod}
							onChange={handleScrubMethodChange}
						/>
					</Card.Content>
				</Card.Root>
			</div>

			<Card.Root class="mt-6">
				<Card.Header>
					<Card.Title>Files to Process</Card.Title>
					<Card.Description
						>{uploadedFiles.length} file(s) ready</Card.Description
					>
				</Card.Header>
				<Card.Content>
					<div class="space-y-2">
						{#each uploadedFiles as file}
							<div
								class="flex items-center justify-between p-3 bg-muted rounded-lg"
							>
								<div class="flex items-center space-x-3">
									<svg
										xmlns="http://www.w3.org/2000/svg"
										class="h-5 w-5 text-muted-foreground"
										viewBox="0 0 20 20"
										fill="currentColor"
									>
										<path
											fill-rule="evenodd"
											d="M4 4a2 2 0 012-2h4.586A2 2 0 0112 2.586L15.414 6A2 2 0 0116 7.414V16a2 2 0 01-2 2H6a2 2 0 01-2-2V4z"
											clip-rule="evenodd"
										/>
									</svg>
									<div>
										<p class="font-medium text-sm">
											{file.name}
										</p>
										<p
											class="text-xs text-muted-foreground"
										>
											{(file.size / 1024).toFixed(1)} KB
										</p>
									</div>
								</div>
								<button
									aria-label="Remove File"
									class="text-muted-foreground hover:text-destructive transition-colors"
									onclick={() => {
										uploadedFiles = uploadedFiles.filter(
											(f) => f.name !== file.name,
										);
										if (uploadedFiles.length === 0)
											currentStep = "upload";
									}}
								>
									<svg
										xmlns="http://www.w3.org/2000/svg"
										class="h-5 w-5"
										viewBox="0 0 20 20"
										fill="currentColor"
									>
										<path
											fill-rule="evenodd"
											d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
											clip-rule="evenodd"
										/>
									</svg>
								</button>
							</div>
						{/each}
					</div>
				</Card.Content>
			</Card.Root>

			<div class="flex justify-between mt-6">
				<Button variant="outline" onclick={goBack}>
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
				<Button
					onclick={processFiles}
					disabled={isProcessing || selectedPiiTypes.length === 0}
				>
					{#if isProcessing}
						<svg
							class="animate-spin -ml-1 mr-2 h-4 w-4"
							xmlns="http://www.w3.org/2000/svg"
							fill="none"
							viewBox="0 0 24 24"
						>
							<circle
								class="opacity-25"
								cx="12"
								cy="12"
								r="10"
								stroke="currentColor"
								stroke-width="4"
							></circle>
							<path
								class="opacity-75"
								fill="currentColor"
								d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
							></path>
						</svg>
						Processing...
					{:else}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							class="h-4 w-4 mr-2"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								fill-rule="evenodd"
								d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z"
								clip-rule="evenodd"
							/>
						</svg>
						Scrub Data
					{/if}
				</Button>
			</div>
		{:else if currentStep === "results"}
			<ResultsPanel
				{results}
				onDownload={downloadFile}
				onDownloadAll={downloadAllFiles}
				onReset={reset}
				onBack={goBack}
			/>
		{/if}
	</main>
</div>
