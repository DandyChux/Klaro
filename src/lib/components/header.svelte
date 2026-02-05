<script lang="ts">
	import { Button } from "$lib/components/ui/button";

	let isDark = false;

	function toggleTheme() {
		isDark = !isDark;
		document.documentElement.classList.toggle("dark", isDark);
	}
</script>

<header class="border-b bg-card">
	<div class="container mx-auto px-4 py-4 max-w-5xl">
		<div class="flex items-center justify-between">
			<div class="flex items-center space-x-3">
				<div
					class="w-10 h-10 bg-primary rounded-lg flex items-center justify-center"
				>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						class="h-6 w-6 text-primary-foreground"
						viewBox="0 0 20 20"
						fill="currentColor"
					>
						<path
							fill-rule="evenodd"
							d="M2.166 4.999A11.954 11.954 0 0010 1.944 11.954 11.954 0 0017.834 5c.11.65.166 1.32.166 2.001 0 5.225-3.34 9.67-8 11.317C5.34 16.67 2 12.225 2 7c0-.682.057-1.35.166-2.001zm11.541 3.708a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
							clip-rule="evenodd"
						/>
					</svg>
				</div>
				<div>
					<h1 class="text-xl font-bold">Rusty Scrubber</h1>
					<p class="text-sm text-muted-foreground">
						Privacy-focused data scrubber
					</p>
				</div>
			</div>
			<Button variant="ghost" size="icon" onclick={toggleTheme}>
				{#if isDark}
					<svg
						xmlns="http://www.w3.org/2000/svg"
						class="h-5 w-5"
						viewBox="0 0 20 20"
						fill="currentColor"
					>
						<path
							fill-rule="evenodd"
							d="M10 2a1 1 0 011 1v1a1 1 0 11-2 0V3a1 1 0 011-1zm4 8a4 4 0 11-8 0 4 4 0 018 0zm-.464 4.95l.707.707a1 1 0 001.414-1.414l-.707-.707a1 1 0 00-1.414 1.414zm2.12-10.607a1 1 0 010 1.414l-.706.707a1 1 0 11-1.414-1.414l.707-.707a1 1 0 011.414 0zM17 11a1 1 0 100-2h-1a1 1 0 100 2h1zm-7 4a1 1 0 011 1v1a1 1 0 11-2 0v-1a1 1 0 011-1zM5.05 6.464A1 1 0 106.465 5.05l-.708-.707a1 1 0 00-1.414 1.414l.707.707zm1.414 8.486l-.707.707a1 1 0 01-1.414-1.414l.707-.707a1 1 0 011.414 1.414zM4 11a1 1 0 100-2H3a1 1 0 000 2h1z"
							clip-rule="evenodd"
						/>
					</svg>
				{:else}
					<svg
						xmlns="http://www.w3.org/2000/svg"
						class="h-5 w-5"
						viewBox="0 0 20 20"
						fill="currentColor"
					>
						<path
							d="M17.293 13.293A8 8 0 016.707 2.707a8.001 8.001 0 1010.586 10.586z"
						/>
					</svg>
				{/if}
			</Button>
		</div>
	</div>
</header>
