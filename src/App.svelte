<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { save } from "@tauri-apps/plugin-dialog";
	import { writeFile } from "@tauri-apps/plugin-fs";
	import { onDestroy, onMount, tick } from "svelte";
	import FileUpload from "$lib/components/file-upload.svelte";
	import PiiOptions from "$lib/components/pii-options.svelte";
	import ResultsPanel from "$lib/components/results-panel.svelte";
	import ScrubMethodSelect from "$lib/components/scrub-method-select.svelte";
	import Header from "$lib/components/header.svelte";
	import { Button } from "$lib/components/ui/button";
	import * as Card from "$lib/components/ui/card";
	import { Separator } from "$lib/components/ui/separator";
	import * as Alert from "$lib/components/ui/alert";
	import Progress from "$lib/components/ui/progress/progress.svelte";
	import { listen, type UnlistenFn } from "@tauri-apps/api/event";

	interface PiiTypeInfo {
		id: string;
		name: string;
		description: string;
		available_in_lite: boolean;
	}

	interface VersionInfo {
		version_name: string;
		is_trial: boolean;
		limits: {
			max_file_size_bytes: number | null;
			max_rows_per_file: number | null;
			max_files_per_batch: number | null;
		};
		features: {
			max_file_size_display: string;
			max_rows_display: string;
			max_files_display: string;
			available_pii_types: string[];
			all_pii_types: string[];
		};
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

	interface ProgressPayload {
		phase: string;
		rows_processed: number;
		total_rows: number;
		pii_found: number;
		percent: number;
	}

	let piiTypes = $state<PiiTypeInfo[]>([]);
	let supportedFormats = $state<SupportedFormat[]>([]);
	let selectedPiiTypes = $state<string[]>([]);
	let scrubMethod = $state<string>("mask");
	let uploadedFiles = $state<UploadedFile[]>([]);
	let results = $state<ProcessResult[]>([]);
	let isProcessing = $state<boolean>(false);
	let isCancelling = $state<boolean>(false);
	let error = $state<string | null>(null);
	let currentStep = $state<"upload" | "configure" | "results">("upload");

	let processingStatus = $state("");
	let processingFileIndex = $state(0);
	let processingPhase = $state<
		"preparing" | "scanning" | "scrubbing" | "finalizing"
	>("preparing");
	let totalPiiFoundSoFar = $state(0);
	let versionInfo = $state<VersionInfo | null>(null);

	// Real progress from Rust backend events
	let backendPercent = $state(0);
	let backendRowsProcessed = $state(0);
	let backendTotalRows = $state(0);

	// Derived progress percentage
	let progressPercent = $derived.by(() => {
		if (uploadedFiles.length === 0) return 0;
		// Weight: progress within the current file + completed files
		const completedFiles = Math.max(0, processingFileIndex - 1);
		const fileWeight = completedFiles / uploadedFiles.length;
		const currentFileWeight =
			(backendPercent / 100) * (1 / uploadedFiles.length);
		return Math.round((fileWeight + currentFileWeight) * 100);
	});

	let unlistenProgress: UnlistenFn | null = null;

	onMount(async () => {
		try {
			// Fetch version info first
			versionInfo = await invoke<VersionInfo>("get_version_info");

			piiTypes = await invoke<PiiTypeInfo[]>("get_pii_types");
			supportedFormats = await invoke<SupportedFormat[]>(
				"get_supported_formats",
			);
			// Select all PII types by default
			selectedPiiTypes = piiTypes
				.filter((t) => t.available_in_lite)
				.map((t) => t.id);
		} catch (e) {
			error = `Failed to initialize: ${e}`;
		}
	});

	onDestroy(() => {
		if (unlistenProgress) {
			unlistenProgress();
			unlistenProgress = null;
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

	/** Wait for the browser to actually paint a frame */
	function waitForPaint(): Promise<void> {
		return new Promise((resolve) => {
			requestAnimationFrame(() => {
				// Double-rAF ensures the frame is
				// committed before we continue
				requestAnimationFrame(() => resolve());
			});
		});
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
		processingFileIndex = 0;
		totalPiiFoundSoFar = 0;
		backendPercent = 0;
		backendRowsProcessed = 0;
		backendTotalRows = 0;
		processingPhase = "preparing";
		processingStatus = "Preparing to process files...";

		// Force UI update before starting heavy work
		await tick();
		await waitForPaint();

		// Listen for real-time progress events from the backend
		unlistenProgress = await listen<ProgressPayload>(
			"processing-progress",
			(event) => {
				const p = event.payload;
				backendPercent = p.percent;
				backendRowsProcessed = p.rows_processed;
				backendTotalRows = p.total_rows;

				totalPiiFoundSoFar =
					results.reduce(
						(sum, r) => sum + (r.stats?.total_pii_found || 0),
						0,
					) + p.pii_found;

				if (
					p.phase === "scanning" ||
					p.phase === "scrubbing" ||
					p.phase === "preparing"
				) {
					processingPhase = p.phase as typeof processingPhase;
				}
			},
		);

		try {
			for (let i = 0; i < uploadedFiles.length; i++) {
				const file = uploadedFiles[i];
				processingFileIndex = i + 1;
				backendPercent = 0;
				processingPhase = "scanning";
				processingStatus = `Processing "${file.name}"...`;

				const result = await invoke<ProcessResult>("process_file", {
					request: {
						file_data: file.data,
						file_name: file.name,
						pii_types: selectedPiiTypes,
						scrub_method: scrubMethod,
					},
				});

				// If cancelled, the backend returns an error result
				if (result.error && result.error.includes("cancelled")) {
					error = "Processing was cancelled";
					break;
				}

				results = [...results, result];
				totalPiiFoundSoFar += result.stats?.total_pii_found || 0;
			}

			if (!error) {
				processingPhase = "finalizing";
				processingStatus = "Complete!";
				await tick();
				currentStep = "results";
			}
		} catch (e) {
			if (`${e}`.includes("cancelled")) {
				error = "Processing was cancelled.";
			} else {
				error = `Processing failed: ${e}`;
			}
		} finally {
			isProcessing = false;
			isCancelling = false;
			processingStatus = "";
			processingPhase = "preparing";
			if (unlistenProgress) {
				unlistenProgress();
				unlistenProgress = null;
			}
		}
	}

	async function cancelProcessing() {
		isCancelling = true;
		try {
			await invoke("cancel_processing");
		} catch (e) {
			console.error("Failed to cancel: ", e);
		}
	}

	function reset() {
		uploadedFiles = [];
		results = [];
		error = null;
		currentStep = "upload";
		totalPiiFoundSoFar = 0;
	}

	function goBack() {
		if (currentStep === "results") {
			currentStep = "configure";
		} else if (currentStep === "configure") {
			currentStep = "upload";
		}
	}

	async function downloadFile(result: ProcessResult) {
		if (!result.scrubbed_data) return;

		try {
			// Open save dialog
			const filePath = await save({
				defaultPath: result.file_name,
				filters: [
					{
						name: "Scrubbed File",
						extensions: [result.file_type || "csv"],
					},
				],
			});

			if (filePath) {
				const binaryString = atob(result.scrubbed_data);
				const bytes = new Uint8Array(binaryString.length);
				for (let i = 0; i < binaryString.length; i++) {
					bytes[i] = binaryString.charCodeAt(i);
				}

				await writeFile(filePath, bytes);
			}
		} catch (e) {
			console.error("Failed to save file: ", e);
			error = `Failed to save file: ${e}`;
		}
	}

	function downloadAllFiles() {
		results.forEach((result) => {
			if (result.success) {
				downloadFile(result);
			}
		});
	}

	function getPhaseIcon(phase: typeof processingPhase) {
		switch (phase) {
			case "preparing":
				return "📋";
			case "scanning":
				return "🔍";
			case "scrubbing":
				return "🧹";
			case "finalizing":
				return "✅";
		}
	}

	function getPhaseText(phase: typeof processingPhase) {
		switch (phase) {
			case "preparing":
				return "Preparing";
			case "scanning":
				return "Scanning for PII";
			case "scrubbing":
				return "Scrubbing data";
			case "finalizing":
				return "Finalizing";
		}
	}
</script>

<!-- Processing Overlay -->
<div
	class="fixed inset-0 z-50 transition-all duration-200 {isProcessing
		? 'opacity-100 pointer-events-auto'
		: 'opacity-0 pointer-events-none'}"
>
	<div class="absolute inset-0 bg-background/80 backdrop-blur-sm"></div>

	<div class="relative h-full flex items-center justify-center p-4">
		<div class="bg-card border rounded-xl p-8 shadow-2xl max-w-lg w-full">
			<!-- Header -->
			<div class="text-center mb-6">
				<div
					class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-primary/10 mb-4"
				>
					<svg
						class="animate-spin h-8 w-8 text-primary"
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
				</div>
				<h3 class="text-xl font-semibold">Processing Your Files</h3>
				<p class="text-muted-foreground text-sm mt-1">
					{isCancelling
						? "Cancelling..."
						: "Please wait while we scrub your data"}
				</p>
			</div>

			<!-- Progress Bar -->
			<div class="mb-6">
				<div class="flex justify-between text-sm mb-2">
					<span class="text-muted-foreground">
						{#if backendTotalRows > 0}
							Row {backendRowsProcessed} of {backendTotalRows}
						{:else}
							Progress
						{/if}
					</span>
					<span class="font-medium">{progressPercent}%</span>
				</div>
				<Progress value={progressPercent} max={100} class="h-3" />
			</div>

			<!-- Current File -->
			<div class="bg-muted/50 rounded-lg p-4 mb-4">
				<div class="flex items-center gap-3">
					<span class="text-2xl">{getPhaseIcon(processingPhase)}</span
					>
					<div class="flex-1 min-w-0">
						<p class="text-sm font-medium text-muted-foreground">
							{getPhaseText(processingPhase)}
						</p>
						<p
							class="font-medium truncate"
							title={processingStatus}
						>
							{processingStatus || "..."}
						</p>
					</div>
				</div>
			</div>

			<!-- Stats -->
			<div class="grid grid-cols-3 gap-3 text-center">
				<div class="bg-muted/30 rounded-lg p-3">
					<p class="text-2xl font-bold text-primary">
						{processingFileIndex}
					</p>
					<p class="text-xs text-muted-foreground">
						of {uploadedFiles.length} files
					</p>
				</div>
				<div class="bg-muted/30 rounded-lg p-3">
					<p class="text-2xl font-bold text-orange-500">
						{totalPiiFoundSoFar}
					</p>
					<p class="text-xs text-muted-foreground">PII found</p>
				</div>
				<div class="bg-muted/30 rounded-lg p-3">
					<p class="text-2xl font-bold text-green-500">
						{results.filter((r) => r.success).length}
					</p>
					<p class="text-xs text-muted-foreground">completed</p>
				</div>
			</div>

			<!-- Cancel Button -->
			<div class="mt-6 text-center">
				<Button
					onclick={cancelProcessing}
					variant="destructive"
					disabled={isCancelling}
					class="transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
				>
					{isCancelling ? "Cancelling..." : "Cancel Processing"}
				</Button>
			</div>

			<!-- Footer Note -->
			<p class="text-xs text-center text-muted-foreground mt-4">
				All processing happens locally on your device
			</p>
		</div>
	</div>
</div>

<div class="min-h-screen bg-background">
	<Header />

	<main class="container mx-auto px-4 py-8 max-w-5xl">
		{#if error}
			<Alert.Root variant="info" class="mb-6">
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
				<Alert.Title>Cancellation</Alert.Title>
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
					class="hover:cursor-pointer"
				>
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
